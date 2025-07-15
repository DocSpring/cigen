use anyhow::{Context, Result};
use serde_json::Value;
use serde_yaml;
use std::path::Path;

use super::schemas::{SchemaRetriever, get_job_schema};

pub struct JobValidator;

impl JobValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_job(&self, job_path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(job_path)
            .with_context(|| format!("Failed to read job file: {job_path:?}"))?;

        let yaml_value: Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML from: {job_path:?}"))?;

        // Parse schema
        let schema = get_job_schema().context("Failed to parse job schema")?;

        // Build validator with our custom retriever for offline validation
        let validator = jsonschema::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
            .context("Failed to compile job schema")?;

        // Validate
        match validator.validate(&yaml_value) {
            Ok(()) => {
                println!("✓ Job validation passed: {job_path:?}");
                Ok(())
            }
            Err(error) => {
                anyhow::bail!("Validation failed for {job_path:?}:\n  - {error}");
            }
        }
    }
}
