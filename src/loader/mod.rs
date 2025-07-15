//! Core loader that validates, loads, merges, and resolves all configuration

mod cache_dependencies;
mod merger;
pub mod span_tracker;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use self::cache_dependencies::infer_cache_dependencies;
use self::merger::ConfigMerger;
use self::span_tracker::SpanTracker;
use crate::models::{Command, Config, Job};
use crate::templating::TemplateEngine;
use crate::validation::{Validator, data::ReferenceValidator};

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
    validator: Validator,
    template_engine: TemplateEngine,
}

impl ConfigLoader {
    pub fn new() -> Result<Self> {
        Ok(Self {
            validator: Validator::new()?,
            template_engine: TemplateEngine::new(),
        })
    }

    pub fn new_with_vars(cli_vars: &HashMap<String, String>) -> Result<Self> {
        let mut template_engine = TemplateEngine::new();
        template_engine.add_env_vars()?;
        template_engine.add_cli_vars(cli_vars);

        Ok(Self {
            validator: Validator::new()?,
            template_engine,
        })
    }

    /// Load and validate all configuration, returning the fully resolved object model
    pub fn load_all(&mut self) -> Result<LoadedConfig> {
        // 1. First validation pass: validate .yml files before template resolution
        // This gives nice miette error messages for IDE compatibility
        // Skip .yml.j2 files since they may not be valid YAML before resolution
        self.validator.validate_all(Path::new("."))?;

        // 2. Load and merge main config (with templating)
        let (config, config_value) = self.load_merged_config()?;

        // Vars are already loaded in load_merged_config during first pass

        // 3. Load all jobs with inheritance applied (with span tracking)
        let mut span_tracker = SpanTracker::new();
        let mut jobs = self.load_all_jobs_with_spans(&config, &mut span_tracker)?;

        // 4. Infer job dependencies from cache usage
        tracing::debug!("Inferring job dependencies from cache usage...");
        infer_cache_dependencies(&mut jobs)?;
        tracing::info!("✓ Job dependencies inferred from cache usage");

        // 5. Load all commands
        let commands = self.load_all_commands()?;

        // 6. Second validation pass: validate rendered YAML for both .yml and .yml.j2 files
        // This ensures the final processed YAML is valid, regardless of file type
        tracing::debug!("Running post-template validation pass...");
        self.validate_rendered_files()?;
        tracing::info!("✓ Post-template validation passed");

        // 7. Validate all data references with span information
        tracing::debug!("Validating all data references...");
        let reference_validator = ReferenceValidator::new();
        reference_validator.validate_all_references(&config, &jobs, &span_tracker)?;
        tracing::info!("✓ All data references validated successfully");

        Ok(LoadedConfig {
            config,
            config_value,
            jobs,
            commands,
        })
    }

    fn load_merged_config(&mut self) -> Result<(Config, serde_json::Value)> {
        // First pass: Load configs without templating to extract vars
        let config_path = Path::new("config.yml");
        let content = std::fs::read_to_string(config_path)?;
        let main_config: serde_json::Value = serde_yaml::from_str(&content)?;

        // Load split configs from config/ directory (without templating)
        let config_dir = Path::new("config");
        let mut split_configs = Vec::new();

        if config_dir.exists() && config_dir.is_dir() {
            for entry in std::fs::read_dir(config_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "yml" || ext == "yaml" || ext == "j2" {
                            let fragment_content = std::fs::read_to_string(&path)?;
                            let fragment: serde_json::Value =
                                serde_yaml::from_str(&fragment_content)?;
                            split_configs.push((path, fragment));
                        }
                    }
                }
            }
        }

        // Merge configs to get vars
        let merged_for_vars = if !split_configs.is_empty() {
            let merger = ConfigMerger::new();
            merger.merge_configs(main_config.clone(), split_configs.clone())?
        } else {
            main_config.clone()
        };

        // Extract and load vars into template engine
        if let Some(vars_value) = merged_for_vars.get("vars") {
            // Convert from serde_json::Value to serde_yaml::Value via string serialization
            let vars_yaml = serde_yaml::to_string(vars_value)?;
            if let Ok(vars) = serde_yaml::from_str::<HashMap<String, serde_yaml::Value>>(&vars_yaml)
            {
                self.template_engine.add_vars_section(&vars);
            }
        }

        // Second pass: Now load configs with templating
        let processed_content = self.process_file_content(config_path, &content)?;
        let main_config: serde_json::Value = serde_yaml::from_str(&processed_content)?;

        // Load split configs from config/ directory (with templating)
        let mut split_configs = Vec::new();

        if config_dir.exists() && config_dir.is_dir() {
            for entry in std::fs::read_dir(config_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "yml" || ext == "yaml" || ext == "j2" {
                            let fragment_content = std::fs::read_to_string(&path)?;
                            let processed_fragment =
                                self.process_file_content(&path, &fragment_content)?;
                            let fragment: serde_json::Value =
                                serde_yaml::from_str(&processed_fragment)?;
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
            let config = Config::from_yaml(&processed_content)?;
            (config, main_config)
        };

        Ok((final_config, final_value))
    }

    fn load_all_jobs_with_spans(
        &mut self,
        _config: &Config,
        span_tracker: &mut SpanTracker,
    ) -> Result<HashMap<String, Job>> {
        let mut jobs = HashMap::new();
        let workflows_dir = Path::new("workflows");

        if !workflows_dir.exists() {
            anyhow::bail!(
                "Missing required 'workflows' directory in {}",
                std::env::current_dir()?.display()
            );
        }

        if !workflows_dir.is_dir() {
            anyhow::bail!("'workflows' must be a directory, not a file");
        }

        // Each subdirectory in workflows should contain a 'jobs' directory
        for entry in std::fs::read_dir(workflows_dir)? {
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
                self.load_jobs_from_directory_with_spans(
                    &jobs_dir,
                    &mut jobs,
                    workflow_name,
                    span_tracker,
                )?;
            }
        }

        Ok(jobs)
    }

    fn load_jobs_from_directory_with_spans(
        &mut self,
        jobs_dir: &Path,
        jobs: &mut HashMap<String, Job>,
        workflow_name: &str,
        span_tracker: &mut SpanTracker,
    ) -> Result<()> {
        for entry in std::fs::read_dir(jobs_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "yml" || ext == "yaml" || ext == "j2" {
                        let content = std::fs::read_to_string(&path)?;
                        let processed_content = self.process_file_content(&path, &content)?;
                        let job: Job = serde_yaml::from_str(&processed_content).map_err(|e| {
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

                        // Track the source file for this job
                        span_tracker.add_job_source(&key, path.clone(), content);

                        jobs.insert(key, job);
                    }
                }
            }
        }

        Ok(())
    }

    fn load_all_commands(&mut self) -> Result<HashMap<String, Command>> {
        let mut commands = HashMap::new();
        let commands_dir = Path::new("commands");

        if commands_dir.exists() {
            for entry in std::fs::read_dir(commands_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "yml" || ext == "yaml" || ext == "j2" {
                            let content = std::fs::read_to_string(&path)?;
                            let processed_content = self.process_file_content(&path, &content)?;
                            let command: Command = serde_yaml::from_str(&processed_content)?;

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

    /// Process file content with templating if needed
    fn process_file_content(&mut self, path: &Path, content: &str) -> Result<String> {
        let is_template = TemplateEngine::is_template_file(path);
        self.template_engine
            .render_file(content, is_template)
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Second validation pass: validate rendered YAML for both .yml and .yml.j2 files
    fn validate_rendered_files(&mut self) -> Result<()> {
        // Validate main config
        let config_path = Path::new("config.yml");
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path)?;
            let rendered = self.process_file_content(config_path, &content)?;
            self.validator
                .validate_config_content(&rendered, config_path)?;
        }

        // Validate split configs - use fragment validation for files in config/ directory
        let config_dir = Path::new("config");
        if config_dir.exists() && config_dir.is_dir() {
            for entry in std::fs::read_dir(config_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "yml" || ext == "yaml" || ext == "j2" {
                            let content = std::fs::read_to_string(&path)?;
                            let rendered = self.process_file_content(&path, &content)?;
                            self.validator
                                .validate_config_fragment_content(&rendered, &path)?;
                        }
                    }
                }
            }
        }

        // Validate job files
        let workflows_dir = Path::new("workflows");
        if workflows_dir.exists() && workflows_dir.is_dir() {
            self.validate_jobs_rendered(workflows_dir)?;
        }

        // Validate command files
        let commands_dir = Path::new("commands");
        if commands_dir.exists() && commands_dir.is_dir() {
            for entry in std::fs::read_dir(commands_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "yml" || ext == "yaml" || ext == "j2" {
                            let content = std::fs::read_to_string(&path)?;
                            let rendered = self.process_file_content(&path, &content)?;
                            self.validator.validate_command_content(&rendered, &path)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate rendered job files recursively
    fn validate_jobs_rendered(&mut self, dir: &Path) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.validate_jobs_rendered(&path)?;
            } else if path.is_file() {
                if let Some(parent) = path.parent() {
                    if let Some(parent_name) = parent.file_name() {
                        if parent_name == "jobs" {
                            if let Some(ext) = path.extension() {
                                if ext == "yml" || ext == "yaml" || ext == "j2" {
                                    let content = std::fs::read_to_string(&path)?;
                                    let rendered = self.process_file_content(&path, &content)?;
                                    self.validator.validate_job_content(&rendered, &path)?;
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
