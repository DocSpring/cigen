use anyhow::{Context, Result};
use miette::Report;
use serde_json::Value;
use serde_yaml;
use std::path::Path;
use tracing::{debug, info};

use super::data::DataValidator;
use super::error_reporter::SpannedValidator;
use super::schemas::{
    SchemaRetriever, get_config_base_schema, get_config_schema, get_workflow_config_schema,
};

// Use JSON Schema draft-07 for validation (stable and well-tested)
// TODO: Upgrade to draft 2020-12 once we update our schemas
use jsonschema::draft7 as schema_draft;

pub struct ConfigValidator;

impl ConfigValidator {
    pub fn new() -> Self {
        Self
    }

    /// Extract a more specific instance path for certain validation errors
    fn refine_instance_path(error: &jsonschema::ValidationError) -> String {
        let error_msg = error.to_string();

        // For "additional properties" errors, extract the property name and append to path
        if error_msg.contains("Additional properties are not allowed") {
            // Extract property names from error message like "('asdf' was unexpected)"
            if let Some(start) = error_msg.find("('")
                && let Some(end) = error_msg[start + 2..].find("'")
            {
                let prop_name = &error_msg[start + 2..start + 2 + end];
                return if error.instance_path.to_string().is_empty() {
                    format!("/{prop_name}")
                } else {
                    format!("{}/{prop_name}", error.instance_path)
                };
            }
        }

        error.instance_path.to_string()
    }

    pub fn validate_config(&self, config_path: &Path) -> Result<()> {
        // 1. Schema validation with beautiful miette error reporting
        let spanned_validator = SpannedValidator::new(config_path)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML from {config_path:?}: {e}"))?;

        let schema = get_config_schema().context("Failed to parse config schema")?;
        let validator = schema_draft::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile config schema")?;

        let errors: Vec<_> = validator
            .iter_errors(spanned_validator.get_json_value())
            .collect();

        if !errors.is_empty() {
            eprintln!(); // Add newline before first error
            for error in &errors {
                let error_msg = error.to_string();
                let instance_path = Self::refine_instance_path(error);

                // Use key span for property-related errors
                let validation_error =
                    if error_msg.contains("Additional properties are not allowed") {
                        spanned_validator.create_error_for_key(&instance_path, error_msg)
                    } else {
                        spanned_validator.create_error(&instance_path, error_msg)
                    };

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

    /// Validate rendered YAML content directly (for post-template validation)
    /// This skips miette error reporting since line numbers won't match original files
    pub fn validate_config_content(&self, yaml_content: &str, source_path: &Path) -> Result<()> {
        let yaml_value: Value = serde_yaml::from_str(yaml_content)
            .with_context(|| format!("Failed to parse rendered YAML from: {source_path:?}"))?;

        let schema = get_config_schema().context("Failed to parse config schema")?;
        let validator = schema_draft::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile config schema")?;

        match validator.validate(&yaml_value) {
            Ok(()) => {
                debug!("✓ Post-template config validation passed: {source_path:?}");
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

    pub fn validate_config_fragment(&self, fragment_path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(fragment_path)
            .with_context(|| format!("Failed to read config fragment: {fragment_path:?}"))?;

        let yaml_value: Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML from: {fragment_path:?}"))?;

        // Use base schema for fragments (allows any subset of properties)
        let schema = get_config_base_schema().context("Failed to parse config base schema")?;

        let validator = schema_draft::options()
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

    /// Validate rendered YAML content as a config fragment (for post-template validation)
    /// This skips miette error reporting since line numbers won't match original files
    pub fn validate_config_fragment_content(
        &self,
        yaml_content: &str,
        source_path: &Path,
    ) -> Result<()> {
        let yaml_value: Value = serde_yaml::from_str(yaml_content)
            .with_context(|| format!("Failed to parse rendered YAML from: {source_path:?}"))?;

        // Use base schema for fragments (allows any subset of properties)
        let schema = get_config_base_schema().context("Failed to parse config base schema")?;
        let validator = schema_draft::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile config base schema")?;

        match validator.validate(&yaml_value) {
            Ok(()) => {
                debug!("✓ Post-template config fragment validation passed: {source_path:?}");
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

    pub fn validate_workflow_config(&self, config_path: &Path) -> Result<()> {
        // 1. Schema validation with beautiful miette error reporting
        let spanned_validator = SpannedValidator::new(config_path)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML from {config_path:?}: {e}"))?;

        let schema =
            get_workflow_config_schema().context("Failed to parse workflow config schema")?;
        let validator = schema_draft::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile workflow config schema")?;

        let errors: Vec<_> = validator
            .iter_errors(spanned_validator.get_json_value())
            .collect();

        if !errors.is_empty() {
            eprintln!(); // Add newline before first error
            for error in &errors {
                let error_msg = error.to_string();
                let instance_path = Self::refine_instance_path(error);

                // Use key span for property-related errors
                let validation_error =
                    if error_msg.contains("Additional properties are not allowed") {
                        spanned_validator.create_error_for_key(&instance_path, error_msg)
                    } else {
                        spanned_validator.create_error(&instance_path, error_msg)
                    };

                eprintln!("{:?}", Report::new(validation_error));
            }
            anyhow::bail!(
                "Schema validation failed for {config_path:?} (see detailed errors above)"
            );
        }

        // Workflow configs don't need data-level validation (no references to validate)
        debug!("    ✓ Workflow config validation passed: {config_path:?}");
        Ok(())
    }

    #[allow(dead_code)]
    pub fn validate_merged(&self, config: &Value) -> Result<()> {
        // Validate against the full schema
        let schema = get_config_schema().context("Failed to parse config schema")?;

        let validator = schema_draft::options()
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
