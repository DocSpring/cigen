/// Plugin manager handles plugin lifecycle and orchestration
///
/// This module is responsible for:
/// - Plugin discovery (PATH, config, hardcoded stdlib)
/// - Process spawning and stdio communication
/// - Handshake with version/capability negotiation
/// - Hook invocation (detect, plan, generate, validate)
/// - Error handling and crash recovery
use crate::plugin::framing::{receive_message, send_message};
use crate::plugin::protocol::{Hello, PluginInfo};
use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

/// Plugin manager coordinates all plugin operations
pub struct PluginManager {
    /// Discovered plugins by name
    pub plugins: HashMap<String, PluginMetadata>,

    /// Active plugin processes
    #[allow(dead_code)]
    active: HashMap<String, PluginProcess>,
}

/// Protocol version that this core supports
const CORE_PROTOCOL_VERSION: u32 = 1;

/// Core version string
const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Handshake timeout (not yet implemented)
#[allow(dead_code)]
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Metadata about a discovered plugin
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub path: PathBuf,
    pub version: String,
    pub protocol: u32,
    pub capabilities: Vec<String>,
}

/// An active plugin process with stdio handles
pub struct PluginProcess {
    pub metadata: PluginMetadata,
    #[allow(dead_code)]
    process: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            active: HashMap::new(),
        }
    }

    /// Discover plugins from PATH and config
    pub async fn discover(&mut self) -> Result<()> {
        // TODO: Implement plugin discovery
        // 1. Check PATH for cigen-provider-*, cigen-lang-*, etc.
        // 2. Check .cigen/plugins/ directory
        // 3. Check config for plugin locations
        Ok(())
    }

    /// Spawn a plugin process and perform handshake
    ///
    /// # Arguments
    /// * `plugin_path` - Path to the plugin binary
    ///
    /// # Returns
    /// Plugin name from the handshake response
    pub async fn spawn<P: AsRef<Path>>(&mut self, plugin_path: P) -> Result<String> {
        let path = plugin_path.as_ref();

        // Spawn the plugin process and perform handshake in a blocking context
        let (process, stdin, stdout, plugin_info) = tokio::task::spawn_blocking({
            let path = path.to_path_buf();
            move || -> Result<(Child, ChildStdin, ChildStdout, PluginInfo)> {
                // Spawn the plugin process
                let mut child = Command::new(&path)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit()) // Plugin logs go to our stderr
                    .spawn()
                    .with_context(|| format!("Failed to spawn plugin: {}", path.display()))?;

                let mut stdin = child
                    .stdin
                    .take()
                    .context("Failed to capture plugin stdin")?;
                let mut stdout = child
                    .stdout
                    .take()
                    .context("Failed to capture plugin stdout")?;

                // Send Hello message
                let hello = Hello {
                    core_protocol: CORE_PROTOCOL_VERSION,
                    core_version: CORE_VERSION.to_string(),
                    env: std::env::vars().collect(),
                };

                send_message(&hello, &mut stdin)
                    .context("Failed to send Hello message to plugin")?;

                // Receive PluginInfo response
                // TODO: Add timeout handling here
                let info: PluginInfo = receive_message(&mut stdout)
                    .context("Failed to receive PluginInfo from plugin")?;

                Ok((child, stdin, stdout, info))
            }
        })
        .await??;

        // Validate protocol version
        if plugin_info.protocol != CORE_PROTOCOL_VERSION {
            bail!(
                "Plugin protocol mismatch: core expects {}, plugin has {}",
                CORE_PROTOCOL_VERSION,
                plugin_info.protocol
            );
        }

        // Create metadata
        let metadata = PluginMetadata {
            name: plugin_info.name.clone(),
            path: path.to_path_buf(),
            version: plugin_info.version.clone(),
            protocol: plugin_info.protocol,
            capabilities: plugin_info.capabilities.clone(),
        };

        tracing::info!(
            "Plugin handshake successful: {} v{} (protocol {})",
            metadata.name,
            metadata.version,
            metadata.protocol
        );
        tracing::debug!("Capabilities: {:?}", metadata.capabilities);

        // Store the active plugin process
        let plugin_name = metadata.name.clone();
        let plugin_process = PluginProcess {
            metadata: metadata.clone(),
            process,
            stdin,
            stdout,
        };

        self.active.insert(plugin_name.clone(), plugin_process);
        self.plugins.insert(plugin_name.clone(), metadata);

        Ok(plugin_name)
    }

    /// Invoke a hook on all plugins with a capability
    pub async fn invoke_hook(&self, _capability: &str, _hook: &str) -> Result<()> {
        // TODO: Implement hook invocation
        // 1. Filter plugins by capability
        // 2. Send hook request to each
        // 3. Collect responses
        // 4. Aggregate results
        Ok(())
    }

    /// Shutdown all active plugins
    ///
    /// Attempts graceful shutdown by closing stdin, then waits for process exit.
    /// If the process doesn't exit within timeout, it will be force-killed.
    pub async fn shutdown(&mut self) -> Result<()> {
        let plugin_names: Vec<String> = self.active.keys().cloned().collect();

        for name in plugin_names {
            if let Some(plugin) = self.active.remove(&name) {
                tracing::info!("Shutting down plugin: {}", name);

                // Clone name for error message since it's moved into closure
                let name_for_error = name.clone();

                // Graceful shutdown in blocking context
                let result = tokio::task::spawn_blocking(move || -> Result<()> {
                    let plugin_name = name;
                    let mut plugin = plugin;

                    // Close stdin to signal plugin to exit
                    drop(plugin.stdin);
                    drop(plugin.stdout);

                    // Wait for process to exit with timeout
                    let timeout = Duration::from_secs(5);
                    let start = std::time::Instant::now();

                    loop {
                        match plugin.process.try_wait()? {
                            Some(status) => {
                                tracing::debug!(
                                    "Plugin {} exited with status: {}",
                                    plugin_name,
                                    status
                                );
                                return Ok(());
                            }
                            None => {
                                if start.elapsed() > timeout {
                                    tracing::warn!(
                                        "Plugin {} did not exit within timeout, force killing",
                                        plugin_name
                                    );
                                    plugin.process.kill()?;
                                    plugin.process.wait()?;
                                    return Ok(());
                                }
                                std::thread::sleep(Duration::from_millis(100));
                            }
                        }
                    }
                })
                .await?;

                if let Err(e) = result {
                    tracing::error!("Failed to shutdown plugin {}: {}", name_for_error, e);
                }
            }
        }

        Ok(())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
