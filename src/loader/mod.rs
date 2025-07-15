//! Core loader that validates, loads, merges, and resolves all configuration

mod merger;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use self::merger::ConfigMerger;
use crate::models::{Command, Config, Job};
use crate::validation::Validator;

/// The fully loaded and resolved configuration system
pub struct LoadedConfig {
    /// The merged and resolved main configuration
    pub config: Config,

    /// The merged config as JSON value (for validation)
    pub config_value: serde_json::Value,

    /// All jobs, keyed by their path (e.g., "test/jobs/rspec")
    pub jobs: HashMap<String, Job>,

    /// All commands, keyed by their name
    pub commands: HashMap<String, Command>,
}

pub struct ConfigLoader {
    base_path: String,
    validator: Validator,
}

impl ConfigLoader {
    pub fn new(base_path: &str) -> Result<Self> {
        Ok(Self {
            base_path: base_path.to_string(),
            validator: Validator::new()?,
        })
    }

    /// Load and validate all configuration, returning the fully resolved object model
    pub fn load_all(&self) -> Result<LoadedConfig> {
        let base_path = Path::new(&self.base_path);

        // 1. First run all validation (this includes schema validation)
        self.validator.validate_all(base_path)?;

        // 2. Load and merge main config
        let (config, config_value) = self.load_merged_config(base_path)?;

        // 3. Load all jobs with inheritance applied
        let jobs = self.load_all_jobs(base_path, &config)?;

        // 4. Load all commands
        let commands = self.load_all_commands(base_path)?;

        // 5. TODO: Apply Tera template resolution to everything

        Ok(LoadedConfig {
            config,
            config_value,
            jobs,
            commands,
        })
    }

    fn load_merged_config(&self, base_path: &Path) -> Result<(Config, serde_json::Value)> {
        // Load main config
        let config_path = base_path.join("config.yml");
        let content = std::fs::read_to_string(&config_path)?;
        let main_config: serde_json::Value = serde_yaml::from_str(&content)?;

        // Load split configs from config/ directory
        let config_dir = base_path.join("config");
        let mut split_configs = Vec::new();

        if config_dir.exists() && config_dir.is_dir() {
            for entry in std::fs::read_dir(&config_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "yml" || ext == "yaml" {
                            let fragment_content = std::fs::read_to_string(&path)?;
                            let fragment: serde_json::Value =
                                serde_yaml::from_str(&fragment_content)?;
                            split_configs.push((path, fragment));
                        }
                    }
                }
            }
        }

        // Merge configs if we have split configs
        let (final_config, final_value) = if !split_configs.is_empty() {
            let merger = ConfigMerger::new();
            let merged = merger.merge_configs(main_config, split_configs)?;
            let merged_yaml = serde_yaml::to_string(&merged)?;
            let config = Config::from_yaml(&merged_yaml)?;
            (config, merged)
        } else {
            let config = Config::from_yaml(&content)?;
            (config, main_config)
        };

        Ok((final_config, final_value))
    }

    fn load_all_jobs(&self, base_path: &Path, _config: &Config) -> Result<HashMap<String, Job>> {
        let mut jobs = HashMap::new();
        let workflows_dir = base_path.join("workflows");

        if !workflows_dir.exists() {
            anyhow::bail!(
                "Missing required 'workflows' directory in {}",
                base_path.display()
            );
        }

        if !workflows_dir.is_dir() {
            anyhow::bail!("'workflows' must be a directory, not a file");
        }

        // Each subdirectory in workflows should contain a 'jobs' directory
        for entry in std::fs::read_dir(&workflows_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let workflow_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                let jobs_dir = path.join("jobs");
                if !jobs_dir.exists() {
                    anyhow::bail!(
                        "Workflow '{}' is missing required 'jobs' directory at: {}",
                        workflow_name,
                        jobs_dir.display()
                    );
                }

                if !jobs_dir.is_dir() {
                    anyhow::bail!(
                        "In workflow '{}', 'jobs' must be a directory, not a file",
                        workflow_name
                    );
                }

                // Load all job files from this jobs directory
                self.load_jobs_from_directory(&jobs_dir, &mut jobs, workflow_name)?;
            }
        }

        Ok(jobs)
    }

    fn load_jobs_from_directory(
        &self,
        jobs_dir: &Path,
        jobs: &mut HashMap<String, Job>,
        workflow_name: &str,
    ) -> Result<()> {
        for entry in std::fs::read_dir(jobs_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "yml" || ext == "yaml" {
                        let content = std::fs::read_to_string(&path)?;
                        let job: Job = serde_yaml::from_str(&content).map_err(|e| {
                            anyhow::anyhow!("Failed to parse job file {}: {}", path.display(), e)
                        })?;

                        let job_name =
                            path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                                anyhow::anyhow!("Invalid job filename: {}", path.display())
                            })?;

                        // Key format: "workflow_name/job_name"
                        let key = format!("{workflow_name}/{job_name}");

                        if jobs.contains_key(&key) {
                            anyhow::bail!("Duplicate job key: {}", key);
                        }

                        jobs.insert(key, job);
                    }
                }
            }
        }

        Ok(())
    }

    fn load_all_commands(&self, base_path: &Path) -> Result<HashMap<String, Command>> {
        let mut commands = HashMap::new();
        let commands_dir = base_path.join("commands");

        if commands_dir.exists() {
            for entry in std::fs::read_dir(&commands_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "yml" || ext == "yaml" {
                            let content = std::fs::read_to_string(&path)?;
                            let command: Command = serde_yaml::from_str(&content)?;

                            let key = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();

                            commands.insert(key, command);
                        }
                    }
                }
            }
        }

        Ok(commands)
    }
}
