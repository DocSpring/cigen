use anyhow::Result;
use std::path::Path;
use tracing::debug;

use super::config::ConfigValidator;
use super::job::JobValidator;
use super::merger::ConfigMerger;

pub struct Validator {
    config_validator: ConfigValidator,
    job_validator: JobValidator,
    merger: ConfigMerger,
}

impl Validator {
    pub fn new() -> Result<Self> {
        Ok(Self {
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
            debug!("Merging configurations...");
            let merged = self.merger.merge_configs(main_config, split_configs)?;

            // Validate the merged config against the full schema
            debug!("Validating merged configuration...");
            self.config_validator.validate_merged(&merged)?;
        }

        // TODO: Validate job files in workflows/
        // TODO: Validate command files
        // TODO: Validate references (services, caches, etc.)

        Ok(())
    }
}
