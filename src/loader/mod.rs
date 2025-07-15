//! Core loader that validates, loads, merges, and resolves all configuration

mod merger;
mod span_tracker;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use self::merger::ConfigMerger;
use self::span_tracker::SpanTracker;
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

        // 3. Load all jobs with inheritance applied (with span tracking)
        let mut span_tracker = SpanTracker::new();
        let jobs = self.load_all_jobs_with_spans(base_path, &config, &mut span_tracker)?;

        // 4. Load all commands
        let commands = self.load_all_commands(base_path)?;

        // 5. TODO: Apply Tera template resolution to everything

        // 6. Validate all data references with span information
        tracing::debug!("Validating all data references...");
        self.validate_all_references_with_spans(&config, &jobs, &commands, &span_tracker)?;
        tracing::info!("✓ All data references validated successfully");

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

    fn load_all_jobs_with_spans(
        &self,
        base_path: &Path,
        _config: &Config,
        span_tracker: &mut SpanTracker,
    ) -> Result<HashMap<String, Job>> {
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
        &self,
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

                        // Track the source file for this job
                        span_tracker.add_job_source(&key, path.clone(), content);

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

    fn validate_all_references_with_spans(
        &self,
        config: &Config,
        jobs: &HashMap<String, Job>,
        _commands: &HashMap<String, Command>,
        span_tracker: &SpanTracker,
    ) -> Result<()> {
        use std::collections::HashSet;

        // First, collect ALL available resources across the entire configuration

        // 1. Services from config
        let available_services: HashSet<&str> = config
            .services
            .as_ref()
            .map(|s| s.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();

        // 2. Source file groups from config
        let available_source_groups: HashSet<&str> = config
            .source_file_groups
            .as_ref()
            .map(|s| s.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();

        // 3. ALL cache definitions from ALL jobs
        let mut all_defined_caches: HashSet<String> = HashSet::new();
        for job in jobs.values() {
            if let Some(cache_defs) = &job.cache {
                for cache_name in cache_defs.keys() {
                    all_defined_caches.insert(cache_name.clone());
                }
            }
        }

        // Now validate each job's references against the global collections
        for (job_key, job) in jobs {
            if let Some(source_info) = span_tracker.get_job_source(job_key) {
                // Re-parse with spans for error reporting
                let spanned_yaml: yaml_spanned::Spanned<yaml_spanned::Value> =
                    yaml_spanned::from_str(&source_info.content)
                        .map_err(|e| anyhow::anyhow!("Failed to parse YAML with spans: {}", e))?;

                let span_finder =
                    crate::validation::data::span_finder::SpanFinder::new(&spanned_yaml);

                // Validate service references
                if let Some(services) = &job.services {
                    for service in services {
                        if !available_services.contains(service.as_str()) {
                            if let Some(span) =
                                span_finder.find_array_item_span(&["services"], service)
                            {
                                let err = crate::validation::data::error::DataValidationError::new(
                                    &source_info.file_path.to_string_lossy(),
                                    source_info.content.clone(),
                                    span,
                                    format!(
                                        "Unknown service '{}'. Available services: {}",
                                        service,
                                        if available_services.is_empty() {
                                            "none defined".to_string()
                                        } else {
                                            available_services
                                                .iter()
                                                .map(|s| format!("'{s}'"))
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        }
                                    ),
                                );
                                eprintln!();
                                eprintln!("{:?}", miette::Report::new(err));
                                return Err(anyhow::anyhow!("Data validation failed"));
                            }
                        }
                    }
                }

                // Validate source file group reference
                if let Some(source_files) = &job.source_files {
                    if !available_source_groups.contains(source_files.as_str()) {
                        if let Some(span) = span_finder.find_field_span(&["source_files"]) {
                            let err = crate::validation::data::error::DataValidationError::new(
                                &source_info.file_path.to_string_lossy(),
                                source_info.content.clone(),
                                span,
                                format!(
                                    "Unknown source file group '{}'. Available groups: {}",
                                    source_files,
                                    if available_source_groups.is_empty() {
                                        "none defined".to_string()
                                    } else {
                                        available_source_groups
                                            .iter()
                                            .map(|s| format!("'{s}'"))
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    }
                                ),
                            );
                            eprintln!();
                            eprintln!("{:?}", miette::Report::new(err));
                            return Err(anyhow::anyhow!("Data validation failed"));
                        }
                    }
                }

                // Validate cache restore references
                if let Some(restore_caches) = &job.restore_cache {
                    for cache_ref in restore_caches {
                        let cache_name = match cache_ref {
                            crate::models::job::CacheRestore::Simple(name) => name,
                            crate::models::job::CacheRestore::Complex { name, .. } => name,
                        };

                        if !all_defined_caches.contains(cache_name) {
                            if let Some(span) = span_finder.find_cache_reference_span(cache_name) {
                                let err = crate::validation::data::error::DataValidationError::new(
                                    &source_info.file_path.to_string_lossy(),
                                    source_info.content.clone(),
                                    span,
                                    format!(
                                        "Unknown cache '{}'. Defined caches: {}",
                                        cache_name,
                                        if all_defined_caches.is_empty() {
                                            "none defined".to_string()
                                        } else {
                                            let mut cache_list: Vec<_> = all_defined_caches
                                                .iter()
                                                .map(|s| s.as_str())
                                                .collect();
                                            cache_list.sort();
                                            cache_list
                                                .iter()
                                                .map(|s| format!("'{s}'"))
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        }
                                    ),
                                );
                                eprintln!();
                                eprintln!("{:?}", miette::Report::new(err));
                                return Err(anyhow::anyhow!("Data validation failed"));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
