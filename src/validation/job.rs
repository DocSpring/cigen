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
        // Parse YAML with span tracking
        let spanned_validator = SpannedValidator::new(job_path)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML from {job_path:?}: {e}"))?;

        // Parse schema
        let schema = get_job_schema().context("Failed to parse job schema")?;

        // Build validator with our custom retriever for offline validation
        let validator = jsonschema::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile job schema")?;

        // Validate with beautiful error reporting
        let errors: Vec<_> = validator
            .iter_errors(spanned_validator.get_json_value())
            .collect();

        if errors.is_empty() {
            debug!("    ✓ Job validation passed: {job_path:?}");
            Ok(())
        } else {
            // Create beautiful error reports
            eprintln!(); // Add newline before first error
            for error in &errors {
                let validation_error = spanned_validator
                    .create_error(&error.instance_path.to_string(), error.to_string());
                eprintln!("{:?}", Report::new(validation_error));
            }

            anyhow::bail!("Validation failed for {job_path:?} (see detailed errors above)");
        }
    }
}
