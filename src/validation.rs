use anyhow::{Context, Result};
use jsonschema::{Retrieve, Uri};
use serde_json::Value;
use serde_yaml;
use std::path::Path;

// Embed schemas at compile time
const CONFIG_SCHEMA: &str = include_str!("../schemas/v1/config-schema.json");
const CONFIG_BASE_SCHEMA: &str = include_str!("../schemas/v1/config-base-schema.json");
const JOB_SCHEMA: &str = include_str!("../schemas/v1/job-schema.json");
const COMMAND_SCHEMA: &str = include_str!("../schemas/v1/command-schema.json");
const DRAFT_07_SCHEMA: &str = include_str!("../schemas/vendor/draft-07-schema.json");

// Custom retriever for embedded schemas
struct SchemaRetriever;

impl Retrieve for SchemaRetriever {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        match uri.as_str() {
            "./config-base-schema.json" => Ok(serde_json::from_str(CONFIG_BASE_SCHEMA)?),
            "https://cigen.dev/schemas/v1/config-base-schema.json" => {
                Ok(serde_json::from_str(CONFIG_BASE_SCHEMA)?)
            }
            "./job-schema.json" => Ok(serde_json::from_str(JOB_SCHEMA)?),
            "https://cigen.dev/schemas/v1/job-schema.json" => Ok(serde_json::from_str(JOB_SCHEMA)?),
            "./command-schema.json" => Ok(serde_json::from_str(COMMAND_SCHEMA)?),
            "https://cigen.dev/schemas/v1/command-schema.json" => {
                Ok(serde_json::from_str(COMMAND_SCHEMA)?)
            }
            "https://json-schema.org/draft-07/schema"
            | "https://json-schema.org/draft-07/schema#"
            | "http://json-schema.org/draft-07/schema"
            | "http://json-schema.org/draft-07/schema#" => {
                Ok(serde_json::from_str(DRAFT_07_SCHEMA)?)
            }
            _ => Err(format!("Unknown schema URI: {uri}").into()),
        }
    }
}

pub struct Validator {}

impl Validator {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub fn validate_config(&self, config_path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {config_path:?}"))?;

        let yaml_value: Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML from: {config_path:?}"))?;

        // Parse schema
        let schema: Value =
            serde_json::from_str(CONFIG_SCHEMA).context("Failed to parse config schema")?;

        // Build validator with our custom retriever for offline validation
        // Use draft7 module directly to avoid meta-schema validation issues
        let validator = match jsonschema::draft7::options()
            .with_retriever(SchemaRetriever)
            .build(&schema)
        {
            Ok(v) => v,
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to compile config schema: {}", e));
            }
        };

        // Validate
        match validator.validate(&yaml_value) {
            Ok(()) => {
                println!("✓ Config validation passed: {config_path:?}");
                Ok(())
            }
            Err(error) => {
                anyhow::bail!("Validation failed for {:?}:\n  - {}", config_path, error);
            }
        }
    }

    pub fn validate_job(&self, job_path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(job_path)
            .with_context(|| format!("Failed to read job file: {job_path:?}"))?;

        let yaml_value: Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML from: {job_path:?}"))?;

        // Parse schema
        let schema: Value =
            serde_json::from_str(JOB_SCHEMA).context("Failed to parse job schema")?;

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
                anyhow::bail!("Validation failed for {:?}:\n  - {}", job_path, error);
            }
        }
    }

    pub fn validate_all(&self, base_path: &Path) -> Result<()> {
        // Validate main config
        let config_path = base_path.join("config.yml");
        if config_path.exists() {
            self.validate_config(&config_path)?;
        }

        // TODO: Validate split configs in config/
        // TODO: Validate job files in workflows/
        // TODO: Validate command files
        // TODO: Validate references (services, caches, etc.)

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validator_creation() {
        let validator = Validator::new();
        assert!(validator.is_ok());
    }

    #[test]
    fn test_validate_minimal_config() {
        let validator = Validator::new().unwrap();

        // Create a temporary directory and config file
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yml");

        // Write minimal valid config
        let minimal_config = r#"
provider: circleci
"#;
        fs::write(&config_path, minimal_config).unwrap();

        // Validate should succeed
        let result = validator.validate_config(&config_path);
        if let Err(e) = &result {
            eprintln!("Validation error: {e}");
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_invalid_config_missing_provider() {
        let validator = Validator::new().unwrap();

        // Create a temporary directory and config file
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yml");

        // Write invalid config (missing required provider)
        let invalid_config = r#"
output_path: ./build
"#;
        fs::write(&config_path, invalid_config).unwrap();

        // Validate should fail
        let result = validator.validate_config(&config_path);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Validation failed"));
    }

    #[test]
    fn test_validate_config_with_services() {
        let validator = Validator::new().unwrap();

        // Create a temporary directory and config file
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yml");

        // Write config with services
        let config_with_services = r#"
provider: circleci

docker:
  default_auth: docker_hub
  auth:
    docker_hub:
      username: $DOCKERHUB_USERNAME
      password: $DOCKERHUB_TOKEN

services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_USER: test
      POSTGRES_DB: test_db
  redis:
    image: redis:7
"#;
        fs::write(&config_path, config_with_services).unwrap();

        // Validate should succeed
        let result = validator.validate_config(&config_path);
        assert!(result.is_ok());
    }
}
