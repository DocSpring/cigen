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
        // Find main config file - check valid locations in order
        let config_path = self.find_main_config_path()?;
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;
        let main_config: JsonValue = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", config_path.display()))?;

        // Load split configs; support running from project root or inside .cigen
        let cwd = std::env::current_dir().unwrap();
        let config_dir = if cwd.file_name() == Some(std::ffi::OsStr::new(".cigen")) {
            Path::new("config")
        } else {
            Path::new(".cigen/config")
        };
        let mut split_configs = Vec::new();

        if config_dir.exists() {
            for path in FileScanner::scan_directory(config_dir)? {
                let content = std::fs::read_to_string(&path)?;
                let fragment: JsonValue = serde_yaml::from_str(&content)?;
                split_configs.push((path, fragment));
            }
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
        let config_path = self.find_main_config_path()?;
        let content = std::fs::read_to_string(&config_path)?;
        let processed = self.process_file_content(&config_path, &content)?;
        let main_config: JsonValue = serde_yaml::from_str(&processed)?;

        // Load split configs with templating; support running from project root or inside .cigen
        let cwd = std::env::current_dir().unwrap();
        let config_dir = if cwd.file_name() == Some(std::ffi::OsStr::new(".cigen")) {
            Path::new("config")
        } else {
            Path::new(".cigen/config")
        };
        let mut split_configs = Vec::new();

        if config_dir.exists() {
            for path in FileScanner::scan_directory(config_dir)? {
                let content = std::fs::read_to_string(&path)?;
                let processed = self.process_file_content(&path, &content)?;
                let fragment: JsonValue = serde_yaml::from_str(&processed)?;
                split_configs.push((path, fragment));
            }
        }

        Ok((main_config, split_configs))
    }

    /// Merge configs and convert to Config model
    fn merge_configs(
        &mut self,
        main_config: JsonValue,
        mut split_configs: Vec<(PathBuf, JsonValue)>,
    ) -> Result<(Config, JsonValue)> {
        // Load workflow definition files and add them to config
        let workflow_configs = self.load_workflow_definitions()?;
        if !workflow_configs.is_empty() {
            // Add workflow configs as a split config entry
            let workflows_json = serde_json::json!({
                "workflows": workflow_configs
            });
            split_configs.push((PathBuf::from(".cigen/workflows"), workflows_json));
        }

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

    /// Load workflow definition files from .cigen/workflows/*.yml
    fn load_workflow_definitions(&mut self) -> Result<HashMap<String, JsonValue>> {
        let cwd = std::env::current_dir().unwrap();
        let workflows_dir = if cwd.file_name() == Some(std::ffi::OsStr::new(".cigen")) {
            Path::new("workflows")
        } else {
            Path::new(".cigen/workflows")
        };

        let mut workflows = HashMap::new();

        if !workflows_dir.exists() {
            return Ok(workflows);
        }

        // Scan for workflow definition files (not job files)
        for entry in std::fs::read_dir(workflows_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Only process .yml/.yaml files directly in the workflows directory
            if path.is_file()
                && (path.extension() == Some(std::ffi::OsStr::new("yml"))
                    || path.extension() == Some(std::ffi::OsStr::new("yaml")))
            {
                let workflow_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Invalid workflow filename: {}", path.display())
                    })?
                    .to_string();

                let content = std::fs::read_to_string(&path)?;
                let processed = self.process_file_content(&path, &content)?;
                let workflow_config: JsonValue = serde_yaml::from_str(&processed)?;

                workflows.insert(workflow_name, workflow_config);
            }
        }

        Ok(workflows)
    }

    /// Find the main config file in valid locations
    fn find_main_config_path(&self) -> Result<PathBuf> {
        // Check the 4 valid locations in order of preference:
        // 1. .cigen/config.yml
        // 2. ./config.yml (root)
        // 3. ./.config.yml (root)
        // 4. If in .cigen/ directory, look for config.yml

        if Path::new(".cigen/config.yml").exists() {
            return Ok(PathBuf::from(".cigen/config.yml"));
        }

        if Path::new("config.yml").exists() {
            return Ok(PathBuf::from("config.yml"));
        }

        if Path::new(".config.yml").exists() {
            return Ok(PathBuf::from(".config.yml"));
        }

        // Check if we're in .cigen directory
        let current_dir = std::env::current_dir().unwrap();
        if current_dir.file_name() == Some(std::ffi::OsStr::new(".cigen"))
            && Path::new("config.yml").exists()
        {
            return Ok(PathBuf::from("config.yml"));
        }

        anyhow::bail!(
            "No config file found. Expected one of: .cigen/config.yml, config.yml, .config.yml, or config.yml (if in .cigen/ directory)"
        )
    }

    /// Process file content with templating if needed
    fn process_file_content(&mut self, path: &Path, content: &str) -> Result<String> {
        let is_template = crate::templating::TemplateEngine::is_template_file(path);
        self.template_engine
            .render_file_with_path(content, path, is_template)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
    }
}
