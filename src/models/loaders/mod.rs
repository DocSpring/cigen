pub mod v1;

use crate::models::{Command, Config, Job};
use anyhow::Result;
use serde_json::Value;

pub struct ConfigLoader;

impl ConfigLoader {
    /// Auto-detect schema version and load config using appropriate loader
    pub fn load_config(content: &str) -> Result<Config> {
        // For now, we only have v1. In the future, we can detect schema version
        // from $schema field or other means and route to appropriate loader
        v1::V1ConfigLoader::load(content)
    }

    /// Load job using current schema (v1 for now)
    pub fn load_job(content: &str) -> Result<Job> {
        v1::V1JobLoader::load(content)
    }

    /// Load command using current schema (v1 for now)
    pub fn load_command(content: &str) -> Result<Command> {
        v1::V1CommandLoader::load(content)
    }

    /// Detect schema version from YAML content
    #[allow(dead_code)]
    fn detect_version(content: &str) -> Result<String> {
        let yaml_value: Value = serde_yaml::from_str(content)?;

        // Look for $schema field to determine version
        if let Some(schema_url) = yaml_value.get("$schema").and_then(|v| v.as_str())
            && schema_url.contains("/v1/")
        {
            return Ok("v1".to_string());
        }
        // Add future version detection here

        // Default to v1 if no version specified
        Ok("v1".to_string())
    }
}
