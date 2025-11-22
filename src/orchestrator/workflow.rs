use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::plugin::manager::PluginManager;
use crate::plugin::protocol::{GenerateRequest, PlanRequest};
use crate::schema::CigenConfig;

use super::convert::config_to_proto;
use super::dag::JobDAG;

/// Main orchestrator for the cigen workflow
pub struct WorkflowOrchestrator {
    /// Plugin manager for spawning and communicating with plugins
    plugin_manager: PluginManager,
    /// Base directory for plugin binaries
    plugin_dir: PathBuf,
}

impl WorkflowOrchestrator {
    /// Create a new workflow orchestrator
    pub fn new(plugin_dir: PathBuf) -> Self {
        Self {
            plugin_manager: PluginManager::new(),
            plugin_dir,
        }
    }

    /// Execute the full workflow: detect → plan → generate → merge
    pub async fn execute(&mut self, mut config: CigenConfig) -> Result<GenerationResult> {
        // 1. Build DAG from job definitions (expands matrix and resolves dependencies)
        let dag = JobDAG::build(&config)
            .context("Failed to build dependency graph from job definitions")?;

        // 2. Reconstruct config with expanded jobs for the plugin
        let mut expanded_jobs = HashMap::new();
        for (instance_id, concrete_job) in dag.jobs() {
            let mut job = concrete_job.job.clone();
            // Ensure matrix is cleared so plugin doesn't try to expand it again
            job.matrix = None;
            // Ensure stage is set to the concrete stage
            job.stage = Some(concrete_job.stage.clone());

            expanded_jobs.insert(instance_id.clone(), job);
        }
        config.jobs = expanded_jobs;

        // 3. Convert config to protobuf
        let proto_schema = config_to_proto(&config);

        // 4. Detect which plugins are needed
        let providers = self.detect_providers(&config);

        // 5. Spawn plugins
        let plugin_ids = self.spawn_plugins(&providers).await?;

        // 6. For each plugin, execute plan → generate workflow
        let mut all_fragments = Vec::new();
        for plugin_id in &plugin_ids {
            // Send PlanRequest
            let plan_request = PlanRequest {
                capabilities: vec![],  // TODO: Collect from all plugins
                facts: HashMap::new(), // TODO: Implement detect phase
                schema: Some(proto_schema.clone()),
                flags: HashMap::new(),
                repo: None, // TODO: Add repository snapshot
            };

            let plan_result = self
                .plugin_manager
                .send_plan(plugin_id, plan_request)
                .await
                .with_context(|| format!("Failed to send plan request to plugin '{plugin_id}'"))?;

            tracing::info!(
                "Plugin '{}' returned {} resources",
                plugin_id,
                plan_result.resources.len()
            );

            // Send GenerateRequest
            let generate_request = GenerateRequest {
                target: extract_provider_name(plugin_id),
                graph: plan_result.resources,
                work_signatures: HashMap::new(), // TODO: Compute work signatures
                schema: Some(proto_schema.clone()),
                facts: HashMap::new(),
            };

            let generate_result = self
                .plugin_manager
                .send_generate(plugin_id, generate_request)
                .await
                .with_context(|| {
                    format!("Failed to send generate request to plugin '{plugin_id}'")
                })?;

            tracing::info!(
                "Plugin '{}' generated {} fragments",
                plugin_id,
                generate_result.fragments.len()
            );

            // Collect fragments
            for fragment in generate_result.fragments {
                let merge_strategy = match fragment.strategy() {
                    crate::plugin::protocol::MergeStrategy::Replace => MergeStrategy::Replace,
                    crate::plugin::protocol::MergeStrategy::Merge => MergeStrategy::Merge,
                    crate::plugin::protocol::MergeStrategy::Append => MergeStrategy::Append,
                    _ => MergeStrategy::Replace,
                };

                all_fragments.push(FileFragment {
                    path: fragment.path,
                    content: fragment.content,
                    merge_strategy,
                });
            }
        }

        // 7. Shutdown all plugins
        self.plugin_manager
            .shutdown()
            .await
            .context("Failed to shutdown plugins")?;

        // 8. Merge fragments and write files
        let files = merge_fragments(all_fragments)?;

        Ok(GenerationResult { files })
    }

    /// Detect which providers are needed from the configuration
    fn detect_providers(&self, config: &CigenConfig) -> Vec<String> {
        if !config.providers.is_empty() {
            return config.providers.clone();
        }

        let mut providers = self.available_providers();
        if providers.is_empty() {
            // Fallback to GitHub provider to match legacy behaviour
            providers.push("github".to_string());
        }
        providers
    }

    /// Spawn all provider plugins
    async fn spawn_plugins(&mut self, providers: &[String]) -> Result<Vec<String>> {
        let mut plugin_ids = Vec::new();

        for provider in providers {
            let plugin_path = self.plugin_dir.join(format!("cigen-provider-{provider}"));

            if !plugin_path.exists() {
                bail!("Plugin binary not found: {}", plugin_path.display());
            }

            let plugin_id = self
                .plugin_manager
                .spawn(&plugin_path)
                .await
                .with_context(|| format!("Failed to spawn plugin for provider '{provider}'"))?;

            plugin_ids.push(plugin_id);
        }

        Ok(plugin_ids)
    }
}

impl WorkflowOrchestrator {
    fn available_providers(&self) -> Vec<String> {
        let mut providers = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.plugin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str())
                    && let Some(stripped) = file_name.strip_prefix("cigen-provider-")
                {
                    providers.push(stripped.to_string());
                }
            }
        }
        providers
    }
}

/// Result of configuration generation
#[derive(Debug)]
pub struct GenerationResult {
    /// Generated files (path -> content)
    pub files: HashMap<String, String>,
}

/// Fragment merge strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Replace existing file
    Replace,
    /// Merge YAML/JSON structures
    Merge,
    /// Append to existing file
    Append,
}

/// A file fragment from a plugin
#[derive(Debug, Clone)]
pub struct FileFragment {
    /// Relative path to write the file
    pub path: String,
    /// File content
    pub content: String,
    /// How to merge with existing content
    pub merge_strategy: MergeStrategy,
}

/// Extract provider name from plugin ID (e.g., "provider/github" -> "github")
fn extract_provider_name(plugin_id: &str) -> String {
    plugin_id
        .split('/')
        .next_back()
        .unwrap_or(plugin_id)
        .to_string()
}

/// Merge fragments into final files
fn merge_fragments(fragments: Vec<FileFragment>) -> Result<HashMap<String, String>> {
    let mut files: HashMap<String, String> = HashMap::new();

    for fragment in fragments {
        match fragment.merge_strategy {
            MergeStrategy::Replace => {
                // Simply replace any existing content
                files.insert(fragment.path, fragment.content);
            }
            MergeStrategy::Append => {
                // Append to existing content
                let content = files.entry(fragment.path).or_default();
                content.push_str(&fragment.content);
            }
            MergeStrategy::Merge => {
                // TODO: Implement YAML/JSON merging
                // For now, just replace
                files.insert(fragment.path, fragment.content);
            }
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_fragments_replace() {
        let fragments = vec![
            FileFragment {
                path: "output.yml".to_string(),
                content: "version: 1".to_string(),
                merge_strategy: MergeStrategy::Replace,
            },
            FileFragment {
                path: "output.yml".to_string(),
                content: "version: 2".to_string(),
                merge_strategy: MergeStrategy::Replace,
            },
        ];

        let files = merge_fragments(fragments).unwrap();
        assert_eq!(files.get("output.yml").unwrap(), "version: 2");
    }

    #[test]
    fn test_merge_fragments_append() {
        let fragments = vec![
            FileFragment {
                path: "output.txt".to_string(),
                content: "line 1\n".to_string(),
                merge_strategy: MergeStrategy::Append,
            },
            FileFragment {
                path: "output.txt".to_string(),
                content: "line 2\n".to_string(),
                merge_strategy: MergeStrategy::Append,
            },
        ];

        let files = merge_fragments(fragments).unwrap();
        assert_eq!(files.get("output.txt").unwrap(), "line 1\nline 2\n");
    }

    #[test]
    fn test_detect_providers() {
        let config = CigenConfig {
            project: None,
            providers: vec!["github".to_string(), "circleci".to_string()],
            packages: vec![],
            source_file_groups: HashMap::new(),
            jobs: HashMap::new(),
            commands: HashMap::new(),
            caches: HashMap::new(),
            runners: HashMap::new(),
            provider_config: HashMap::new(),
            workflows: HashMap::new(),
            raw: Default::default(),
        };

        let orchestrator = WorkflowOrchestrator::new(PathBuf::from("plugins"));
        let providers = orchestrator.detect_providers(&config);

        assert_eq!(providers, vec!["github", "circleci"]);
    }

    #[test]
    fn test_detect_providers_defaults() {
        let config = CigenConfig {
            project: None,
            providers: vec![], // Empty - should use defaults
            packages: vec![],
            source_file_groups: HashMap::new(),
            jobs: HashMap::new(),
            commands: HashMap::new(),
            caches: HashMap::new(),
            runners: HashMap::new(),
            provider_config: HashMap::new(),
            workflows: HashMap::new(),
            raw: Default::default(),
        };

        let orchestrator = WorkflowOrchestrator::new(PathBuf::from("plugins"));
        let providers = orchestrator.detect_providers(&config);

        assert_eq!(providers, vec!["github"]);
    }
}
