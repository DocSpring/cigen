use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::plugin::manager::PluginManager;
use crate::schema::CigenConfig;

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
    pub async fn execute(&mut self, config: CigenConfig) -> Result<GenerationResult> {
        // 1. Build DAG from job definitions
        let _dag = JobDAG::build(&config)
            .context("Failed to build dependency graph from job definitions")?;

        // 2. Detect which plugins are needed
        let providers = self.detect_providers(&config);

        // 3. Spawn plugins
        let plugin_ids = self.spawn_plugins(&providers).await?;

        // 4. For each plugin, execute plan → generate workflow
        let all_fragments = Vec::new();
        for _plugin_id in &plugin_ids {
            // TODO: Send PlanRequest with config and DAG
            // TODO: Receive PlanResponse
            // TODO: Send GenerateRequest
            // TODO: Receive GenerateResponse with fragments
            // TODO: Collect fragments
        }

        // 5. Shutdown all plugins
        self.plugin_manager
            .shutdown()
            .await
            .context("Failed to shutdown plugins")?;

        // 6. Merge fragments and write files
        // TODO: Implement fragment merging
        let files = merge_fragments(all_fragments)?;

        Ok(GenerationResult { files })
    }

    /// Detect which providers are needed from the configuration
    fn detect_providers(&self, config: &CigenConfig) -> Vec<String> {
        config
            .get_providers()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Spawn all provider plugins
    async fn spawn_plugins(&mut self, providers: &[String]) -> Result<Vec<String>> {
        let mut plugin_ids = Vec::new();

        for provider in providers {
            let plugin_path = self.plugin_dir.join(format!("provider-{provider}"));

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
            jobs: HashMap::new(),
            caches: HashMap::new(),
            runners: HashMap::new(),
            provider_config: HashMap::new(),
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
            jobs: HashMap::new(),
            caches: HashMap::new(),
            runners: HashMap::new(),
            provider_config: HashMap::new(),
        };

        let orchestrator = WorkflowOrchestrator::new(PathBuf::from("plugins"));
        let providers = orchestrator.detect_providers(&config);

        assert_eq!(providers, vec!["github", "circleci", "buildkite"]);
    }
}
