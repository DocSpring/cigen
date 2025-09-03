//! Config-specific loading logic

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::file_scanner::FileScanner;
use super::merger::ConfigMerger;
use crate::models::Config;
use crate::templating::TemplateEngine;

pub struct ConfigLoader<'a> {
    template_engine: &'a mut TemplateEngine,
}

impl<'a> ConfigLoader<'a> {
    pub fn new(template_engine: &'a mut TemplateEngine) -> Self {
        Self { template_engine }
    }

    /// Load and merge the main configuration with split configs
    pub fn load_merged_config(&mut self) -> Result<(Config, JsonValue)> {
        // First pass: Load configs without templating to extract vars
        let (main_config, split_configs) = self.load_configs_for_vars()?;

        // Extract and register vars
        self.extract_and_register_vars(&main_config, &split_configs)?;

        // Second pass: Load configs with templating
        let (main_config, split_configs) = self.load_configs_with_templating()?;

        // Merge and return final config
        self.merge_configs(main_config, split_configs)
    }

    /// Load configs without templating (for extracting vars)
    fn load_configs_for_vars(&self) -> Result<(JsonValue, Vec<(PathBuf, JsonValue)>)> {
        // Load main config - check if we're in .cigen directory or need to look for .cigen/cigen.yml
        let current_dir = std::env::current_dir().unwrap();
        let is_in_cigen_dir = current_dir.file_name() == Some(std::ffi::OsStr::new(".cigen"));

        let config_path = if is_in_cigen_dir {
            Path::new("cigen.yml")
        } else if Path::new(".cigen/cigen.yml").exists() {
            Path::new(".cigen/cigen.yml")
        } else {
            Path::new("config.yml")
        };
        let content = std::fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;
        let main_config: JsonValue = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", config_path.display()))?;

        // Load split configs - only look in .cigen/config or config if we're in .cigen
        let config_dir = if is_in_cigen_dir {
            Path::new("config")
        } else {
            Path::new(".cigen/config")
        };
        let mut split_configs = Vec::new();

        for path in FileScanner::scan_directory(config_dir)? {
            let content = std::fs::read_to_string(&path)?;
            let fragment: JsonValue = serde_yaml::from_str(&content)?;
            split_configs.push((path, fragment));
        }

        Ok((main_config, split_configs))
    }

    /// Extract vars from merged config and register them
    fn extract_and_register_vars(
        &mut self,
        main_config: &JsonValue,
        split_configs: &[(PathBuf, JsonValue)],
    ) -> Result<()> {
        // Merge configs to get complete vars
        let merged = if !split_configs.is_empty() {
            let merger = ConfigMerger::new();
            merger.merge_configs(main_config.clone(), split_configs.to_vec())?
        } else {
            main_config.clone()
        };

        // Extract and register vars
        if let Some(vars_value) = merged.get("vars") {
            // Convert from serde_json::Value to serde_yaml::Value
            let vars_yaml = serde_yaml::to_string(vars_value)?;
            if let Ok(vars) = serde_yaml::from_str::<HashMap<String, YamlValue>>(&vars_yaml) {
                self.template_engine.add_vars_section(&vars);
            }
        }

        Ok(())
    }

    /// Load configs with templating applied
    fn load_configs_with_templating(&mut self) -> Result<(JsonValue, Vec<(PathBuf, JsonValue)>)> {
        // Load main config with templating
        let current_dir = std::env::current_dir().unwrap();
        let is_in_cigen_dir = current_dir.file_name() == Some(std::ffi::OsStr::new(".cigen"));

        let config_path = if is_in_cigen_dir {
            Path::new("cigen.yml")
        } else if Path::new(".cigen/cigen.yml").exists() {
            Path::new(".cigen/cigen.yml")
        } else {
            Path::new("config.yml")
        };
        let content = std::fs::read_to_string(config_path)?;
        let processed = self.process_file_content(config_path, &content)?;
        let main_config: JsonValue = serde_yaml::from_str(&processed)?;

        // Load split configs with templating - only look in .cigen/config or config if we're in .cigen
        let config_dir = if is_in_cigen_dir {
            Path::new("config")
        } else {
            Path::new(".cigen/config")
        };
        let mut split_configs = Vec::new();

        // Only process if directory exists
        if !config_dir.exists() {
            return Ok((main_config, split_configs));
        }

        for path in FileScanner::scan_directory(config_dir)? {
            let content = std::fs::read_to_string(&path)?;
            let processed = self.process_file_content(&path, &content)?;
            let fragment: JsonValue = serde_yaml::from_str(&processed)?;
            split_configs.push((path, fragment));
        }

        Ok((main_config, split_configs))
    }

    /// Merge configs and convert to Config model
    fn merge_configs(
        &self,
        main_config: JsonValue,
        split_configs: Vec<(PathBuf, JsonValue)>,
    ) -> Result<(Config, JsonValue)> {
        if !split_configs.is_empty() {
            let merger = ConfigMerger::new();
            let merged = merger.merge_configs(main_config, split_configs)?;
            let merged_yaml = serde_yaml::to_string(&merged)?;
            let config = Config::from_yaml(&merged_yaml)?;
            let config = crate::defaults::merge_with_defaults(config);
            Ok((config, merged))
        } else {
            let yaml = serde_yaml::to_string(&main_config)?;
            let config = Config::from_yaml(&yaml)?;
            let config = crate::defaults::merge_with_defaults(config);
            Ok((config, main_config))
        }
    }

    /// Process file content with templating if needed
    fn process_file_content(&mut self, path: &Path, content: &str) -> Result<String> {
        let is_template = crate::templating::TemplateEngine::is_template_file(path);
        self.template_engine
            .render_file_with_path(content, path, is_template)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
    }
}
