use super::*;
use crate::models::config::ServiceEnvironment;
use crate::models::job::Step;
use crate::models::{Config, Job};
use crate::providers::Provider;
use std::collections::HashMap;

#[test]
fn test_simple_job_conversion() {
    let config = Config::default();

    let mut jobs = HashMap::new();
    jobs.insert(
        "test".to_string(),
        Job {
            image: "cimg/ruby:3.2".to_string(),
            architectures: None,
            resource_class: Some("medium".to_string()),
            source_files: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            steps: Some(vec![
                Step::Command("echo 'Hello World'".to_string()),
                Step::Run {
                    name: Some("Run tests".to_string()),
                    run: "bundle exec rspec".to_string(),
                },
            ]),
        },
    );

    let provider = CircleCIProvider::new();

    // Test through the provider interface by writing to a temp directory
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path();

    let result = provider.generate_workflow(&config, "test_workflow", &jobs, output_path);
    if let Err(e) = &result {
        eprintln!("Error: {e}");
    }
    assert!(result.is_ok());

    // Read and parse the generated YAML file
    let config_file = output_path.join("config.yml");
    assert!(config_file.exists());

    let yaml_content = std::fs::read_to_string(config_file).unwrap();
    let circleci_config: config::CircleCIConfig = serde_yaml::from_str(&yaml_content).unwrap();
    assert_eq!(circleci_config.version, 2.1);
    assert!(circleci_config.workflows.contains_key("test_workflow"));
    assert!(circleci_config.jobs.contains_key("test"));

    // Check job structure
    let job = &circleci_config.jobs["test"];
    assert!(job.docker.is_some());
    assert_eq!(job.resource_class, Some("medium".to_string()));

    // Should have 2 steps (no automatic checkout)
    assert_eq!(job.steps.len(), 2);
}

#[test]
fn test_job_with_services() {
    let mut services = HashMap::new();
    services.insert(
        "postgres".to_string(),
        crate::models::Service {
            image: "postgres:15".to_string(),
            environment: Some(ServiceEnvironment::Map({
                let mut env = HashMap::new();
                env.insert("POSTGRES_PASSWORD".to_string(), "test".to_string());
                env
            })),
            ports: None,
            volumes: None,
            auth: None,
        },
    );

    let config = Config {
        services: Some(services),
        ..Config::default()
    };

    let mut jobs = HashMap::new();
    jobs.insert(
        "test".to_string(),
        Job {
            image: "cimg/ruby:3.2".to_string(),
            services: Some(vec!["postgres".to_string()]),
            architectures: None,
            resource_class: None,
            source_files: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            steps: Some(vec![Step::Command("echo 'Testing with DB'".to_string())]),
        },
    );

    let generator = generator::CircleCIGenerator::new();
    let result = generator.build_config(&config, "test_workflow", &jobs);
    assert!(result.is_ok());

    let circleci_config = result.unwrap();
    let job = &circleci_config.jobs["test"];

    // Should have primary container + service container
    let docker_images = job.docker.as_ref().unwrap();
    assert_eq!(docker_images.len(), 2);

    // Check service container has environment variables
    let postgres_image = &docker_images[1];
    assert_eq!(postgres_image.image, "postgres:15");
    assert!(postgres_image.environment.is_some());
    let env = postgres_image.environment.as_ref().unwrap();
    assert_eq!(env.get("POSTGRES_PASSWORD"), Some(&"test".to_string()));
}

#[test]
fn test_job_dependencies() {
    let config = Config::default();

    let mut jobs = HashMap::new();
    jobs.insert(
        "build".to_string(),
        Job {
            image: "cimg/ruby:3.2".to_string(),
            architectures: None,
            resource_class: None,
            source_files: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            steps: Some(vec![Step::Command("echo 'Building'".to_string())]),
        },
    );

    jobs.insert(
        "test".to_string(),
        Job {
            image: "cimg/ruby:3.2".to_string(),
            requires: Some(crate::models::job::JobRequires::Single("build".to_string())),
            architectures: None,
            resource_class: None,
            source_files: None,
            parallelism: None,
            cache: None,
            restore_cache: None,
            services: None,
            steps: Some(vec![Step::Command("echo 'Testing'".to_string())]),
        },
    );

    let generator = generator::CircleCIGenerator::new();
    let result = generator.build_config(&config, "ci", &jobs);
    assert!(result.is_ok());

    let circleci_config = result.unwrap();
    let workflow = &circleci_config.workflows["ci"];

    // Check workflow job dependencies
    let mut found_test_with_requires = false;
    for job in &workflow.jobs {
        if let config::CircleCIWorkflowJob::Detailed { job } = job {
            if job.contains_key("test") {
                let details = &job["test"];
                assert_eq!(details.requires, Some(vec!["build".to_string()]));
                found_test_with_requires = true;
            }
        }
    }
    assert!(found_test_with_requires);
}
