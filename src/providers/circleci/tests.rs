use super::*;
use crate::models::config::ServiceEnvironment;
use crate::models::job::Step;
use crate::models::{Config, Job};
use crate::providers::Provider;
use serde_yaml::{Mapping, Value};
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
            packages: None,
            steps: Some(vec![
                // Run step with just command
                Step({
                    let mut run_step = Mapping::new();
                    run_step.insert(
                        Value::String("run".to_string()),
                        Value::String("echo 'Hello World'".to_string()),
                    );
                    Value::Mapping(run_step)
                }),
                // Run step with name and command
                Step({
                    let mut run_step = Mapping::new();
                    let mut run_details = Mapping::new();
                    run_details.insert(
                        Value::String("name".to_string()),
                        Value::String("Run tests".to_string()),
                    );
                    run_details.insert(
                        Value::String("command".to_string()),
                        Value::String("bundle exec rspec".to_string()),
                    );
                    run_step.insert(
                        Value::String("run".to_string()),
                        Value::Mapping(run_details),
                    );
                    Value::Mapping(run_step)
                }),
            ]),
            checkout: None,
            job_type: None,
        },
    );

    let provider = CircleCIProvider::new();

    // Test through the provider interface by writing to a temp directory
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path();

    let commands = HashMap::new();
    let result =
        provider.generate_workflow(&config, "test_workflow", &jobs, &commands, output_path);
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

    // Should have 3 steps (checkout + 2 user steps)
    assert_eq!(job.steps.len(), 3);
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
            packages: None,
            steps: Some(vec![Step({
                let mut run_step = Mapping::new();
                run_step.insert(
                    Value::String("run".to_string()),
                    Value::String("echo 'Testing with DB'".to_string()),
                );
                Value::Mapping(run_step)
            })]),
            checkout: None,
            job_type: None,
        },
    );

    let generator = generator::CircleCIGenerator::new();
    let commands = HashMap::new();
    let result = generator.build_config(&config, "test_workflow", &jobs, &commands);
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
            packages: None,
            steps: Some(vec![Step({
                let mut run_step = Mapping::new();
                run_step.insert(
                    Value::String("run".to_string()),
                    Value::String("echo 'Building'".to_string()),
                );
                Value::Mapping(run_step)
            })]),
            checkout: None,
            job_type: None,
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
            packages: None,
            steps: Some(vec![Step({
                let mut run_step = Mapping::new();
                run_step.insert(
                    Value::String("run".to_string()),
                    Value::String("echo 'Testing'".to_string()),
                );
                Value::Mapping(run_step)
            })]),
            checkout: None,
            job_type: None,
        },
    );

    let generator = generator::CircleCIGenerator::new();
    let commands = HashMap::new();
    let result = generator.build_config(&config, "ci", &jobs, &commands);
    assert!(result.is_ok());

    let circleci_config = result.unwrap();
    let workflow = &circleci_config.workflows["ci"];

    // Check workflow job dependencies
    let mut found_test_with_requires = false;
    for job in &workflow.jobs {
        if let config::CircleCIWorkflowJob::Detailed { job } = job
            && job.contains_key("test")
        {
            let details = &job["test"];
            assert_eq!(details.requires, Some(vec!["build".to_string()]));
            found_test_with_requires = true;
        }
    }
    assert!(found_test_with_requires);
}

#[test]
fn test_docker_image_resolution() {
    use crate::models::DockerImageConfig;
    use std::collections::HashMap;

    // Create config with docker_images definitions
    let mut docker_images = HashMap::new();

    // Ruby image with architecture variants
    let mut ruby_architectures = HashMap::new();
    ruby_architectures.insert("amd64".to_string(), "cimg/ruby:3.3.5".to_string());
    ruby_architectures.insert("arm64".to_string(), "cimg/ruby:3.3.5-arm64".to_string());

    docker_images.insert(
        "ruby".to_string(),
        DockerImageConfig {
            default: "cimg/ruby:3.3.5".to_string(),
            architectures: Some(ruby_architectures),
        },
    );

    let config = Config {
        docker_images: Some(docker_images),
        ..Default::default()
    };

    let provider = CircleCIProvider::new();

    // Create a job that references docker image by logical name
    let job = Job {
        image: "ruby".to_string(), // Logical reference, not full image
        architectures: None,
        resource_class: None,
        source_files: None,
        parallelism: None,
        requires: None,
        cache: None,
        restore_cache: None,
        services: None,
        packages: None,
        steps: None,
        checkout: None,
        job_type: None,
    };

    // Convert job and check that the image was resolved
    let circleci_job = provider
        .generator
        .convert_job_with_architecture(&config, &job, "amd64")
        .unwrap();

    // Check that docker images were set
    assert!(circleci_job.docker.is_some());
    let docker_images = circleci_job.docker.unwrap();
    assert_eq!(docker_images.len(), 1);

    // Check that the logical name "ruby" was resolved to actual image
    assert_eq!(docker_images[0].image, "cimg/ruby:3.3.5");
}

#[test]
fn test_docker_image_full_reference_passthrough() {
    let config = Config::default();
    let provider = CircleCIProvider::new();

    // Create a job with full docker image reference
    let job = Job {
        image: "postgres:14.9".to_string(), // Full image reference
        architectures: None,
        resource_class: None,
        source_files: None,
        parallelism: None,
        requires: None,
        cache: None,
        restore_cache: None,
        services: None,
        packages: None,
        steps: None,
        checkout: None,
        job_type: None,
    };

    // Convert job
    let circleci_job = provider
        .generator
        .convert_job_with_architecture(&config, &job, "amd64")
        .unwrap();

    // Check that full image reference was used as-is
    let docker_images = circleci_job.docker.unwrap();
    assert_eq!(docker_images[0].image, "postgres:14.9");
}

#[test]
fn test_architecture_matrix_generation() {
    let config = Config::default();

    let mut jobs = HashMap::new();
    jobs.insert(
        "test".to_string(),
        Job {
            image: "cimg/ruby:3.3.5".to_string(),
            architectures: Some(vec!["amd64".to_string(), "arm64".to_string()]),
            resource_class: None,
            source_files: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            steps: None,
            checkout: None,
            job_type: None,
        },
    );

    let generator = generator::CircleCIGenerator::new();
    let commands = HashMap::new();
    let result = generator.build_config(&config, "test_workflow", &jobs, &commands);
    if let Err(e) = &result {
        eprintln!("Error: {e}");
    }
    assert!(result.is_ok());

    let circleci_config = result.unwrap();

    // Should have generated two job variants
    assert!(circleci_config.jobs.contains_key("test_amd64"));
    assert!(circleci_config.jobs.contains_key("test_arm64"));

    // Check workflow contains both jobs
    let workflow = &circleci_config.workflows["test_workflow"];
    let job_names: Vec<String> = workflow
        .jobs
        .iter()
        .map(|job| match job {
            config::CircleCIWorkflowJob::Simple(name) => name.clone(),
            config::CircleCIWorkflowJob::Detailed { job } => job.keys().next().unwrap().clone(),
        })
        .collect();

    assert!(job_names.contains(&"test_amd64".to_string()));
    assert!(job_names.contains(&"test_arm64".to_string()));
}

#[test]
fn test_architecture_matrix_with_dependencies() {
    let config = Config::default();

    let mut jobs = HashMap::new();

    // Build job with architectures
    jobs.insert(
        "build".to_string(),
        Job {
            image: "cimg/ruby:3.3.5".to_string(),
            architectures: Some(vec!["amd64".to_string(), "arm64".to_string()]),
            resource_class: None,
            source_files: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            steps: None,
            checkout: None,
            job_type: None,
        },
    );

    // Test job that depends on build
    jobs.insert(
        "test".to_string(),
        Job {
            image: "cimg/ruby:3.3.5".to_string(),
            architectures: Some(vec!["amd64".to_string(), "arm64".to_string()]),
            requires: Some(crate::models::job::JobRequires::Single("build".to_string())),
            resource_class: None,
            source_files: None,
            parallelism: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            steps: None,
            checkout: None,
            job_type: None,
        },
    );

    let generator = generator::CircleCIGenerator::new();
    let commands = HashMap::new();
    let result = generator.build_config(&config, "test_workflow", &jobs, &commands);
    assert!(result.is_ok());

    let circleci_config = result.unwrap();

    // Should have generated architecture variants for both jobs
    assert!(circleci_config.jobs.contains_key("build_amd64"));
    assert!(circleci_config.jobs.contains_key("build_arm64"));
    assert!(circleci_config.jobs.contains_key("test_amd64"));
    assert!(circleci_config.jobs.contains_key("test_arm64"));

    // Check workflow dependencies are correctly mapped
    let workflow = &circleci_config.workflows["test_workflow"];

    // Find test_amd64 job and check its dependencies
    for job in &workflow.jobs {
        if let config::CircleCIWorkflowJob::Detailed { job } = job
            && job.contains_key("test_amd64")
        {
            let details = &job["test_amd64"];
            assert_eq!(details.requires, Some(vec!["build_amd64".to_string()]));
        }
        if let config::CircleCIWorkflowJob::Detailed { job } = job
            && job.contains_key("test_arm64")
        {
            let details = &job["test_arm64"];
            assert_eq!(details.requires, Some(vec!["build_arm64".to_string()]));
        }
    }
}

#[test]
fn test_single_architecture_no_suffix() {
    let config = Config::default();

    let mut jobs = HashMap::new();
    jobs.insert(
        "test".to_string(),
        Job {
            image: "cimg/ruby:3.3.5".to_string(),
            architectures: Some(vec!["amd64".to_string()]), // Single architecture
            resource_class: None,
            source_files: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            steps: None,
            checkout: None,
            job_type: None,
        },
    );

    let generator = generator::CircleCIGenerator::new();
    let commands = HashMap::new();
    let result = generator.build_config(&config, "test_workflow", &jobs, &commands);
    assert!(result.is_ok());

    let circleci_config = result.unwrap();

    // Should have only one job without architecture suffix
    assert!(circleci_config.jobs.contains_key("test"));
    assert!(!circleci_config.jobs.contains_key("test_amd64"));
}

#[test]
fn test_architecture_environment_variables() {
    use crate::models::DockerImageConfig;
    use std::collections::HashMap;

    // Create config with docker_images definitions
    let mut docker_images = HashMap::new();
    let mut ruby_architectures = HashMap::new();
    ruby_architectures.insert("amd64".to_string(), "cimg/ruby:3.3.5".to_string());
    ruby_architectures.insert("arm64".to_string(), "cimg/ruby:3.3.5-arm64".to_string());

    docker_images.insert(
        "ruby".to_string(),
        DockerImageConfig {
            default: "cimg/ruby:3.3.5".to_string(),
            architectures: Some(ruby_architectures),
        },
    );

    let config = Config {
        docker_images: Some(docker_images),
        ..Default::default()
    };

    let job = Job {
        image: "ruby".to_string(),
        architectures: Some(vec!["arm64".to_string()]),
        resource_class: None,
        source_files: None,
        parallelism: None,
        requires: None,
        cache: None,
        restore_cache: None,
        services: None,
        packages: None,
        steps: None,
        checkout: None,
        job_type: None,
    };

    let generator = generator::CircleCIGenerator::new();
    let circleci_job = generator
        .convert_job_with_architecture(&config, &job, "arm64")
        .unwrap();

    // Check environment variables
    let env = circleci_job.environment.unwrap();
    assert_eq!(env.get("DOCKER_ARCH").unwrap(), "arm64");

    // Check that the correct architecture-specific image was used
    let docker_images = circleci_job.docker.unwrap();
    assert_eq!(docker_images[0].image, "cimg/ruby:3.3.5-arm64");
}

#[test]
fn test_dynamic_config_with_parameters() {
    use crate::models::ParameterConfig;
    use serde_json::Value as JsonValue;

    // Create config with dynamic setup and parameters
    let mut parameters = HashMap::new();
    parameters.insert(
        "run_tests".to_string(),
        ParameterConfig {
            param_type: "boolean".to_string(),
            default: Some(JsonValue::Bool(true)),
            description: Some("Whether to run tests".to_string()),
        },
    );
    parameters.insert(
        "environment".to_string(),
        ParameterConfig {
            param_type: "string".to_string(),
            default: Some(JsonValue::String("production".to_string())),
            description: Some("Target environment".to_string()),
        },
    );

    let config = Config {
        setup: Some(true),
        parameters: Some(parameters),
        ..Default::default()
    };

    let mut jobs = HashMap::new();
    jobs.insert(
        "build".to_string(),
        Job {
            image: "cimg/ruby:3.3.5".to_string(),
            architectures: None,
            resource_class: None,
            source_files: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            steps: None,
            checkout: None,
            job_type: None,
        },
    );

    let generator = generator::CircleCIGenerator::new();
    let commands = HashMap::new();
    let result = generator.build_config(&config, "setup", &jobs, &commands);
    assert!(result.is_ok());

    let circleci_config = result.unwrap();

    // Check that setup is enabled
    assert_eq!(circleci_config.setup, Some(true));

    // Check parameters are converted correctly
    let params = circleci_config.parameters.unwrap();
    assert!(params.contains_key("run_tests"));
    assert!(params.contains_key("environment"));

    // Check boolean parameter
    if let config::CircleCIParameter::Boolean {
        param_type,
        default,
        description,
    } = &params["run_tests"]
    {
        assert_eq!(param_type, "boolean");
        assert_eq!(default, &Some(true));
        assert_eq!(description, &Some("Whether to run tests".to_string()));
    } else {
        panic!("Expected boolean parameter");
    }

    // Check string parameter
    if let config::CircleCIParameter::String {
        param_type,
        default,
        description,
    } = &params["environment"]
    {
        assert_eq!(param_type, "string");
        assert_eq!(default, &Some("production".to_string()));
        assert_eq!(description, &Some("Target environment".to_string()));
    } else {
        panic!("Expected string parameter");
    }
}

#[test]
fn test_dynamic_flag_enables_setup() {
    let config = Config {
        dynamic: Some(true),
        ..Default::default()
    };

    let mut jobs = HashMap::new();
    jobs.insert(
        "build".to_string(),
        Job {
            image: "cimg/ruby:3.3.5".to_string(),
            architectures: None,
            resource_class: None,
            source_files: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            steps: None,
            checkout: None,
            job_type: None,
        },
    );

    let generator = generator::CircleCIGenerator::new();
    let commands = HashMap::new();
    let result = generator.build_config(&config, "setup", &jobs, &commands);
    assert!(result.is_ok());

    let circleci_config = result.unwrap();

    // Check that setup is enabled when dynamic is true
    assert_eq!(circleci_config.setup, Some(true));
}
