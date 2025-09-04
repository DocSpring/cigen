use super::Validator;
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
    assert!(error_msg.contains("Schema validation failed"));
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

#[test]
fn test_validate_all_missing_config() {
    let validator = Validator::new().unwrap();

    // Create a temporary directory without config.yml
    let temp_dir = TempDir::new().unwrap();

    // Validate should fail with no config file found error
    let result = validator.validate_all(temp_dir.path());
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("No config file found"));
}

#[test]
fn test_validate_all_with_split_configs() {
    let validator = Validator::new().unwrap();

    // Create a temporary directory structure with proper .cigen layout
    let temp_dir = TempDir::new().unwrap();
    let cigen_dir = temp_dir.path().join(".cigen");
    let config_dir = cigen_dir.join("config");
    fs::create_dir_all(&config_dir).unwrap();

    // Write main config in .cigen/config.yml location
    let main_config = r#"
provider: circleci
output_path: ./build
"#;
    fs::write(cigen_dir.join("config.yml"), main_config).unwrap();

    // Write split config files in .cigen/config/
    let services_config = r#"
services:
  postgres:
    image: postgres:15
  redis:
    image: redis:7
"#;
    fs::write(config_dir.join("services.yml"), services_config).unwrap();

    let docker_config = r#"
docker:
  default_auth: docker_hub
  auth:
    docker_hub:
      username: $DOCKERHUB_USERNAME
      password: $DOCKERHUB_TOKEN
"#;
    fs::write(config_dir.join("docker.yml"), docker_config).unwrap();

    // Validate should succeed
    let result = validator.validate_all(temp_dir.path());
    assert!(result.is_ok());
}
