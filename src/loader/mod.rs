//! Core loader that validates, loads, merges, and resolves all configuration

mod cache_dependencies;
mod command_loader;
mod config_loader;
pub mod file_scanner;
mod job_loader;
mod merger;
pub mod span_tracker;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use self::cache_dependencies::infer_cache_dependencies;
use self::command_loader::CommandLoader;
use self::config_loader::ConfigLoader as ConfigFileLoader;
use self::job_loader::JobLoader;
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
        let mut config_loader = ConfigFileLoader::new(&mut self.template_engine);
        let (config, config_value) = config_loader.load_merged_config()?;

        // 3. Load all jobs with inheritance applied (with span tracking)
        let mut span_tracker = SpanTracker::new();
        let mut job_loader = JobLoader::new(&mut self.template_engine);
        let mut jobs = job_loader.load_all_jobs(&config, &mut span_tracker)?;

        // 4. Infer job dependencies from cache usage
        tracing::debug!("Inferring job dependencies from cache usage...");
        infer_cache_dependencies(&mut jobs)?;
        tracing::info!("✓ Job dependencies inferred from cache usage");

        // 5. Load all commands
        let mut command_loader = CommandLoader::new(&mut self.template_engine);
        let commands = command_loader.load_all_commands()?;

        // 6. Second validation pass: validate rendered YAML for both .yml and .yml.j2 files
        // This ensures the final processed YAML is valid, regardless of file type
        self.validator
            .validate_rendered_files(&mut self.template_engine)?;

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
}
