use anyhow::{Context, Result};
use miette::Report;
use std::path::Path;
use tracing::debug;

use super::error_reporter::SpannedValidator;
use super::schemas::{SchemaRetriever, get_job_schema};

pub struct JobValidator;

impl JobValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_job(&self, job_path: &Path) -> Result<()> {
        // 1. Schema validation with beautiful error reporting
        let spanned_validator = SpannedValidator::new(job_path)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML from {job_path:?}: {e}"))?;

        let schema = get_job_schema().context("Failed to parse job schema")?;
        let validator = jsonschema::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile job schema")?;

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
            anyhow::bail!("Schema validation failed for {job_path:?} (see detailed errors above)");
        }

        // Note: Data-level validation (service references, cache references, etc.)
        // should happen AFTER template processing, not on raw YAML files.
        // Raw job files may contain MiniJinja variables that need to be resolved first.
        //
        // However, we still need to preserve span information from the original YAML
        // so we can show beautiful miette errors pointing to the source locations
        // when validating the final merged structs.

        debug!("    ✓ Job validation passed: {job_path:?}");
        Ok(())
    }

    /// Validate rendered YAML content directly (for post-template validation)
    /// This skips miette error reporting since line numbers won't match original files
    pub fn validate_job_content(&self, yaml_content: &str, source_path: &Path) -> Result<()> {
        let yaml_value: serde_json::Value = serde_yaml::from_str(yaml_content)
            .with_context(|| format!("Failed to parse rendered YAML from: {source_path:?}"))?;

        let schema = get_job_schema().context("Failed to parse job schema")?;
        let validator = jsonschema::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile job schema")?;

        match validator.validate(&yaml_value) {
            Ok(()) => {
                debug!("✓ Post-template job validation passed: {source_path:?}");
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
