use super::*;
use std::fs;

#[test]
fn test_load_example_config() {
    let content = fs::read_to_string("integration_tests/circleci_rails/.cigen/config.yml").unwrap();
    let config = ConfigLoader::load_config(&content).unwrap();

    assert_eq!(config.provider, "circleci");
    assert!(config.output_path.is_some());
}

#[test]
fn test_load_example_job() {
    let content = fs::read_to_string(
        "integration_tests/circleci_node_simple_split/.cigen/workflows/test/jobs/lint.yml",
    )
    .unwrap();
    let job = ConfigLoader::load_job(&content).unwrap();

    assert_eq!(job.image, "cimg/node:18.20");
    assert!(job.parallelism.is_none());
}

#[test]
fn test_load_example_command() {
    let content =
        fs::read_to_string("integration_tests/circleci_rails/.cigen/commands/setup_database.yml")
            .unwrap();
    let command = ConfigLoader::load_command(&content).unwrap();

    assert_eq!(command.description, "Setup Test Database");
    assert_eq!(command.steps.len(), 1);
}
