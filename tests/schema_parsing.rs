/// Tests for parsing example cigen.yml configs
use cigen::schema::{CigenConfig, JobMatrix};
use std::fs;

#[test]
fn test_parse_minimal_example() {
    let yaml = fs::read_to_string("examples/minimal/cigen.yml").unwrap();
    let config = CigenConfig::from_yaml(&yaml).unwrap();

    assert_eq!(config.jobs.len(), 1);
    assert!(config.jobs.contains_key("test"));

    let test_job = &config.jobs["test"];
    assert_eq!(test_job.packages.len(), 1);
    assert_eq!(test_job.packages[0].name, "ruby");
    assert_eq!(test_job.steps.len(), 1);
}

#[test]
fn test_parse_rails_app_example() {
    let yaml = fs::read_to_string("examples/rails-app/cigen.yml").unwrap();
    let config = CigenConfig::from_yaml(&yaml).unwrap();

    assert!(config.project.is_some());
    assert_eq!(config.project.as_ref().unwrap().name, "myapp");

    // Check jobs
    assert!(config.jobs.contains_key("setup"));
    assert!(config.jobs.contains_key("test"));
    assert!(config.jobs.contains_key("build"));
    assert!(config.jobs.contains_key("deploy-staging"));

    // Check test job has matrix
    let test_job = &config.jobs["test"];
    if let Some(JobMatrix::Dimensions(dims)) = &test_job.matrix {
        assert!(!dims.is_empty());
        assert!(dims.contains_key("ruby"));
        assert!(dims.contains_key("arch"));
    } else {
        panic!("Expected Dimensions matrix for test job");
    }

    // Check services
    assert_eq!(test_job.services.len(), 2);
    assert!(test_job.services.contains(&"postgres:15".to_string()));
}

#[test]
fn test_parse_monorepo_example() {
    let yaml = fs::read_to_string("examples/monorepo/cigen.yml").unwrap();
    let config = CigenConfig::from_yaml(&yaml).unwrap();

    assert!(config.project.is_some());
    let project = config.project.as_ref().unwrap();
    assert_eq!(project.name, "monorepo");

    // Check global packages
    assert_eq!(config.packages, vec!["node"]);

    // Check jobs
    assert!(config.jobs.contains_key("lint"));
    assert!(config.jobs.contains_key("test"));
    assert!(config.jobs.contains_key("e2e"));
}

#[test]
fn test_parse_multi_provider_example() {
    let yaml = fs::read_to_string("examples/multi-provider/cigen.yml").unwrap();
    let config = CigenConfig::from_yaml(&yaml).unwrap();

    // Check providers
    assert_eq!(config.providers, vec!["github", "circleci", "buildkite"]);

    // Check provider_config exists
    assert!(config.provider_config.contains_key("github"));
    assert!(config.provider_config.contains_key("circleci"));
    assert!(config.provider_config.contains_key("buildkite"));
}

#[test]
fn test_job_dependency_validation() {
    let yaml = r#"
jobs:
  test:
    needs:
      - setup
  setup: {}
"#;

    // Should succeed - setup is defined
    let config = CigenConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.jobs["test"].needs, vec!["setup"]);
}

#[test]
fn test_matrix_dimensions() {
    let yaml = r#"
jobs:
  test:
    matrix:
      ruby:
        - "3.2"
        - "3.3"
      node:
        - "18"
        - "20"
"#;

    let config = CigenConfig::from_yaml(yaml).unwrap();
    let test_job = &config.jobs["test"];

    if let Some(JobMatrix::Dimensions(dims)) = &test_job.matrix {
        assert_eq!(dims.len(), 2);
        assert!(dims.contains_key("ruby"));
        assert!(dims.contains_key("node"));
    } else {
        panic!("Expected Dimensions matrix for test job");
    }
}
