use anyhow::{Context, Result};
use serde_json::Value;
use serde_yaml;
use std::path::Path;
use tracing::debug;

use super::schemas::{SchemaRetriever, get_command_schema};

pub struct CommandValidator;

impl CommandValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_command(&self, command_path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(command_path)
            .with_context(|| format!("Failed to read command file: {command_path:?}"))?;

        let yaml_value: Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML from: {command_path:?}"))?;

        // Parse schema
        let schema = get_command_schema().context("Failed to parse command schema")?;

        // Build validator with our custom retriever for offline validation
        let validator = jsonschema::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile command schema")?;

        // Validate
        match validator.validate(&yaml_value) {
            Ok(()) => {
                debug!("    ✓ Command validation passed: {command_path:?}");
                Ok(())
            }
            Err(error) => {
                anyhow::bail!("Validation failed for {command_path:?}:\n  - {error}");
            }
        }
    }

    /// Validate rendered YAML content directly (for post-template validation)
    /// This skips miette error reporting since line numbers won't match original files
    pub fn validate_command_content(&self, yaml_content: &str, source_path: &Path) -> Result<()> {
        let yaml_value: Value = serde_yaml::from_str(yaml_content)
            .with_context(|| format!("Failed to parse rendered YAML from: {source_path:?}"))?;

        let schema = get_command_schema().context("Failed to parse command schema")?;
        let validator = jsonschema::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile command schema")?;

        match validator.validate(&yaml_value) {
            Ok(()) => {
                debug!("✓ Post-template command validation passed: {source_path:?}");
                Ok(())
            }
            Err(error) => {
                anyhow::bail!(
                    "Post-template schema validation failed for {source_path:?}: {}",
                    error
                );
            }
        }
    }
}
