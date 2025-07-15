use super::*;
use std::fs;

#[test]
fn test_load_example_config() {
    let content = fs::read_to_string("examples/circleci_rails/config.yml").unwrap();
    let config = ConfigLoader::load_config(&content).unwrap();

    assert_eq!(config.provider, "circleci");
    assert!(config.output_path.is_some());
}

#[test]
fn test_load_example_job() {
    let content =
        fs::read_to_string("examples/circleci_rails/workflows/test/jobs/rspec.yml").unwrap();
    let job = ConfigLoader::load_job(&content).unwrap();

    assert_eq!(job.image, "ci_app_base");
    assert!(job.parallelism.is_some());
    assert_eq!(job.parallelism.unwrap(), 2);

    // Test service references
    let service_refs = job.service_references();
    assert!(service_refs.contains(&&"postgres".to_string()));
    assert!(service_refs.contains(&&"redis".to_string()));
}

#[test]
fn test_load_example_command() {
    let content =
        fs::read_to_string("examples/circleci_rails/commands/setup_database.yml").unwrap();
    let command = ConfigLoader::load_command(&content).unwrap();

    assert_eq!(command.description, "Setup Test Database");
    assert!(!command.steps.is_empty());
}
