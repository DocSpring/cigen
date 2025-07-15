use anyhow::Result;
use std::path::Path;
use tracing::{debug, info};

use super::command::CommandValidator;
use super::config::ConfigValidator;
use super::job::JobValidator;
use super::merger::ConfigMerger;

pub struct Validator {
    command_validator: CommandValidator,
    config_validator: ConfigValidator,
    job_validator: JobValidator,
    merger: ConfigMerger,
}

impl Validator {
    pub fn new() -> Result<Self> {
        Ok(Self {
            command_validator: CommandValidator::new(),
            config_validator: ConfigValidator::new(),
            job_validator: JobValidator::new(),
            merger: ConfigMerger::new(),
        })
    }

    pub fn validate_config(&self, config_path: &Path) -> Result<()> {
        self.config_validator.validate_config(config_path)
    }

    pub fn validate_job(&self, job_path: &Path) -> Result<()> {
        self.job_validator.validate_job(job_path)
    }

    pub fn validate_all(&self, base_path: &Path) -> Result<()> {
        // Main config.yml is required
        let config_path = base_path.join("config.yml");
        if !config_path.exists() {
            anyhow::bail!(
                "Missing required config.yml in {:?}. Even with split configs, a root config.yml is required.",
                base_path
            );
        }

        // First, validate the main config
        debug!("Validating main config...");
        self.config_validator.validate_config(&config_path)?;

        // Load main config for merging
        let main_config = self.config_validator.load_yaml(&config_path)?;

        // Then validate split configs in config/ directory
        let config_dir = base_path.join("config");
        let mut split_configs = Vec::new();

        if config_dir.exists() && config_dir.is_dir() {
            debug!("Validating split configs...");
            for entry in std::fs::read_dir(&config_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "yml" || ext == "yaml" {
                            debug!("  Validating {:?}...", path.file_name().unwrap());

                            // Validate against base schema (allows partial configs)
                            self.config_validator.validate_config_fragment(&path)?;

                            // Load for merging
                            let fragment = self.config_validator.load_yaml(&path)?;
                            split_configs.push((path.clone(), fragment));
                        }
                    }
                }
            }
        }

        // Merge all configs together
        if !split_configs.is_empty() {
            info!("✓ All split configs validated: {config_dir:?}");

            debug!("Merging configurations...");
            let merged = self.merger.merge_configs(main_config, split_configs)?;

            // Validate the merged config against the full schema
            debug!("Validating merged configuration...");
            self.config_validator.validate_merged(&merged)?;
        }

        // Validate job files in workflows/
        let workflows_dir = base_path.join("workflows");
        if workflows_dir.exists() && workflows_dir.is_dir() {
            debug!("Validating job files...");
            let job_count = self.validate_jobs_in_directory(&workflows_dir)?;
            if job_count > 0 {
                info!("✓ All {} job files validated successfully", job_count);
            }
        }

        // Validate command files in commands/
        let commands_dir = base_path.join("commands");
        if commands_dir.exists() && commands_dir.is_dir() {
            debug!("Validating command files...");
            let command_count = self.validate_commands_in_directory(&commands_dir)?;
            if command_count > 0 {
                info!(
                    "✓ All {} command files validated successfully",
                    command_count
                );
            }
        }

        // TODO: Validate references (services, caches, etc.)

        Ok(())
    }

    fn validate_jobs_in_directory(&self, dir: &Path) -> Result<usize> {
        use std::fs;
        let mut job_count = 0;

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recursively validate subdirectories
                job_count += self.validate_jobs_in_directory(&path)?;
            } else if path.is_file() {
                // Check if it's a YAML file in a jobs/ directory
                if let Some(parent) = path.parent() {
                    if let Some(parent_name) = parent.file_name() {
                        if parent_name == "jobs" {
                            if let Some(ext) = path.extension() {
                                if ext == "yml" || ext == "yaml" {
                                    debug!("  Validating job {:?}...", path.file_name().unwrap());
                                    self.job_validator.validate_job(&path)?;
                                    job_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(job_count)
    }

    fn validate_commands_in_directory(&self, dir: &Path) -> Result<usize> {
        use std::fs;
        let mut command_count = 0;

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "yml" || ext == "yaml" {
                        debug!("  Validating command {:?}...", path.file_name().unwrap());
                        self.command_validator.validate_command(&path)?;
                        command_count += 1;
                    }
                }
            }
        }

        Ok(command_count)
    }
}
