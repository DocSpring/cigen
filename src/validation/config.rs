use anyhow::{Context, Result};
use miette::Report;
use serde_json::Value;
use serde_yaml;
use std::path::Path;
use tracing::{debug, info};

use super::data::DataValidator;
use super::error_reporter::SpannedValidator;
use super::schemas::{SchemaRetriever, get_config_base_schema, get_config_schema};

pub struct ConfigValidator;

impl ConfigValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_config(&self, config_path: &Path) -> Result<()> {
        // 1. Schema validation with beautiful miette error reporting
        let spanned_validator = SpannedValidator::new(config_path)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML from {config_path:?}: {e}"))?;

        let schema = get_config_schema().context("Failed to parse config schema")?;
        let validator = jsonschema::draft7::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile config schema")?;

        let errors: Vec<_> = validator
            .iter_errors(spanned_validator.get_json_value())
            .collect();

        if !errors.is_empty() {
            eprintln!(); // Add newline before first error
            for error in &errors {
                let validation_error = spanned_validator
                    .create_error(&error.instance_path.to_string(), error.to_string());
                eprintln!("{:?}", Report::new(validation_error));
            }
            anyhow::bail!(
                "Schema validation failed for {config_path:?} (see detailed errors above)"
            );
        }

        // 2. Data-level validation with span tracking and miette errors
        debug!("Running data-level validation");
        let content = std::fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {config_path:?}"))?;
        let data_validator = DataValidator::new(config_path, content);
        let _config = data_validator
            .validate_config_data()
            .with_context(|| format!("Data validation failed for {config_path:?}"))?;

        info!("✓ Config validation passed: {config_path:?}");
        Ok(())
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

    #[allow(dead_code)]
    pub fn validate_merged(&self, config: &Value) -> Result<()> {
        // Validate against the full schema
        let schema = get_config_schema().context("Failed to parse config schema")?;

        let validator = jsonschema::draft7::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile config schema")?;

        match validator.validate(config) {
            Ok(()) => {
                info!("✓ Merged config validation passed");
                Ok(())
            }
            Err(error) => {
                anyhow::bail!("Merged configuration validation failed:\n  - {error}");
            }
        }
    }
}
