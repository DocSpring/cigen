use anyhow::{Context, Result};
use serde_json::Value;
use serde_yaml;
use std::path::Path;
use tracing::{debug, info};

use super::schemas::{SchemaRetriever, get_config_base_schema, get_config_schema};

pub struct ConfigValidator;

impl ConfigValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_config(&self, config_path: &Path) -> Result<()> {
        debug!("Reading config file: {config_path:?}");
        let content = std::fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {config_path:?}"))?;

        debug!("Parsing YAML content ({} bytes)", content.len());
        let yaml_value: Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML from: {config_path:?}"))?;

        // Parse schema
        let schema = get_config_schema().context("Failed to parse config schema")?;

        // Build validator with our custom retriever for offline validation
        // Use draft7 module directly to avoid meta-schema validation issues
        let validator = jsonschema::draft7::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile config schema")?;

        // Validate
        debug!("Running schema validation");
        match validator.validate(&yaml_value) {
            Ok(()) => {
                info!("✓ Config validation passed: {config_path:?}");
                Ok(())
            }
            Err(error) => {
                anyhow::bail!("Validation failed for {config_path:?}:\n  - {error}");
            }
        }
    }

    pub fn validate_config_fragment(&self, fragment_path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(fragment_path)
            .with_context(|| format!("Failed to read config fragment: {fragment_path:?}"))?;

        let yaml_value: Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML from: {fragment_path:?}"))?;

        // Use base schema for fragments (allows any subset of properties)
        let schema = get_config_base_schema().context("Failed to parse config base schema")?;

        let validator = jsonschema::draft7::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile config base schema")?;

        match validator.validate(&yaml_value) {
            Ok(()) => {
                debug!("    ✓ Fragment validation passed");
                Ok(())
            }
            Err(error) => {
                anyhow::bail!("Validation failed for {fragment_path:?}:\n  - {error}");
            }
        }
    }

    pub fn validate_merged(&self, config: &Value) -> Result<()> {
        // Validate against the full schema
        let schema = get_config_schema().context("Failed to parse config schema")?;

        let validator = jsonschema::draft7::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile config schema")?;

        match validator.validate(config) {
            Ok(()) => {
                debug!("✓ Merged config validation passed");
                Ok(())
            }
            Err(error) => {
                anyhow::bail!("Merged configuration validation failed:\n  - {error}");
            }
        }
    }

    pub fn load_yaml(&self, path: &Path) -> Result<Value> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {path:?}"))?;
        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML from: {path:?}"))
    }
}
