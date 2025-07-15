//! Post-template validation of rendered files

use anyhow::Result;
use std::path::Path;

use super::Validator;
use crate::loader::file_scanner::FileScanner;
use crate::templating::TemplateEngine;

pub struct PostTemplateValidator<'a> {
    validator: &'a Validator,
    template_engine: &'a mut TemplateEngine,
}

impl<'a> PostTemplateValidator<'a> {
    pub fn new(validator: &'a Validator, template_engine: &'a mut TemplateEngine) -> Self {
        Self {
            validator,
            template_engine,
        }
    }

    /// Run post-template validation on all rendered files
    pub fn validate_all(&mut self) -> Result<()> {
        tracing::debug!("Running post-template validation pass...");

        self.validate_main_config()?;
        self.validate_split_configs()?;
        self.validate_job_files()?;
        self.validate_command_files()?;

        tracing::info!("✓ Post-template validation passed");
        Ok(())
    }

    /// Validate the main config file
    fn validate_main_config(&mut self) -> Result<()> {
        let config_path = Path::new("config.yml");
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path)?;
            let rendered = self.process_file_content(config_path, &content)?;
            self.validator
                .validate_config_content(&rendered, config_path)?;
        }
        Ok(())
    }

    /// Validate split config files
    fn validate_split_configs(&mut self) -> Result<()> {
        let config_dir = Path::new("config");

        for path in FileScanner::scan_directory(config_dir)? {
            let content = std::fs::read_to_string(&path)?;
            let rendered = self.process_file_content(&path, &content)?;
            self.validator
                .validate_config_fragment_content(&rendered, &path)?;
        }

        Ok(())
    }

    /// Validate job files
    fn validate_job_files(&mut self) -> Result<()> {
        let workflows_dir = Path::new("workflows");

        if workflows_dir.exists() && workflows_dir.is_dir() {
            for (job_path, _workflow_name) in FileScanner::scan_job_files(workflows_dir)? {
                let content = std::fs::read_to_string(&job_path)?;
                let rendered = self.process_file_content(&job_path, &content)?;
                self.validator.validate_job_content(&rendered, &job_path)?;
            }
        }

        Ok(())
    }

    /// Validate command files
    fn validate_command_files(&mut self) -> Result<()> {
        let commands_dir = Path::new("commands");

        for path in FileScanner::scan_directory(commands_dir)? {
            let content = std::fs::read_to_string(&path)?;
            let rendered = self.process_file_content(&path, &content)?;
            self.validator.validate_command_content(&rendered, &path)?;
        }

        Ok(())
    }

    /// Process file content with templating if needed
    fn process_file_content(&mut self, path: &Path, content: &str) -> Result<String> {
        let is_template = TemplateEngine::is_template_file(path);
        self.template_engine
            .render_file(content, is_template)
            .map_err(|e| anyhow::anyhow!(e))
    }
}
