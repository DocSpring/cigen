use crate::models::job::CacheRestore;
use crate::models::{Config, Job};
use crate::providers::circleci::generator::CircleCIGenerator;

#[test]
fn test_cache_restoration() {
    let generator = CircleCIGenerator::new();
    let config = Config::default();

    // Create a job with cache restoration
    let job = Job {
        image: "rust:latest".to_string(),
        architectures: None,
        resource_class: Some("medium".to_string()),
        source_files: None,
        parallelism: None,
        requires: None,
        cache: None,
        restore_cache: Some(vec![
            CacheRestore::Simple("gems".to_string()),
            CacheRestore::Complex {
                name: "npm_packages".to_string(),
                dependency: Some(false),
            },
        ]),
        services: None,
        packages: None,
        steps: None,
        checkout: None,
        job_type: None,
    };

    let circleci_job = generator
        .convert_job_with_architecture(&config, &job, "amd64", "test_job_amd64")
        .unwrap();

    // Verify checkout + cache restore steps are added
    assert!(
        circleci_job.steps.len() >= 3,
        "Should have checkout + at least 2 restore_cache steps"
    );

    // Check checkout step
    let checkout_step = &circleci_job.steps[0];
    let checkout_yaml = serde_yaml::to_string(&checkout_step.raw).unwrap();
    assert!(
        checkout_yaml.contains("checkout"),
        "First step should be checkout"
    );

    // Check first cache restore step (now at index 1)
    let first_cache_step = &circleci_job.steps[1];
    let step_yaml = serde_yaml::to_string(&first_cache_step.raw).unwrap();
    assert!(
        step_yaml.contains("restore_cache"),
        "Second step should be restore_cache"
    );
    assert!(step_yaml.contains("gems"), "First cache should be for gems");

    // Check second cache restore step (now at index 2)
    let second_cache_step = &circleci_job.steps[2];
    let step_yaml = serde_yaml::to_string(&second_cache_step.raw).unwrap();
    assert!(
        step_yaml.contains("restore_cache"),
        "Third step should be restore_cache"
    );
    assert!(
        step_yaml.contains("npm_packages"),
        "Second cache should be for npm_packages"
    );
}

#[test]
fn test_job_with_cache_and_steps() {
    let generator = CircleCIGenerator::new();
    let config = Config::default();

    // Create a job with both cache restoration and regular steps
    let job = Job {
        image: "node:14".to_string(),
        architectures: None,
        resource_class: Some("large".to_string()),
        source_files: None,
        parallelism: None,
        requires: None,
        cache: None,
        restore_cache: Some(vec![CacheRestore::Simple("node_modules".to_string())]),
        services: None,
        packages: None,
        steps: Some(vec![
            crate::models::job::Step(
                serde_yaml::from_str(
                    r#"
                run:
                  name: Install Dependencies
                  command: npm install
            "#,
                )
                .unwrap(),
            ),
            crate::models::job::Step(
                serde_yaml::from_str(
                    r#"
                run:
                  name: Run Tests
                  command: npm test
            "#,
                )
                .unwrap(),
            ),
        ]),
        checkout: None,
        job_type: None,
    };

    let circleci_job = generator
        .convert_job_with_architecture(&config, &job, "amd64", "test_job_amd64")
        .unwrap();

    // Verify checkout + cache restore step comes before regular steps
    assert_eq!(
        circleci_job.steps.len(),
        4,
        "Should have checkout + 1 cache restore + 2 regular steps"
    );

    // First step should be checkout
    let checkout_yaml = serde_yaml::to_string(&circleci_job.steps[0].raw).unwrap();
    assert!(
        checkout_yaml.contains("checkout"),
        "First step should be checkout"
    );

    // Second step should be restore_cache
    let cache_yaml = serde_yaml::to_string(&circleci_job.steps[1].raw).unwrap();
    assert!(
        cache_yaml.contains("restore_cache"),
        "Second step should be restore_cache"
    );

    // Third step should be Install Dependencies
    let install_yaml = serde_yaml::to_string(&circleci_job.steps[2].raw).unwrap();
    assert!(
        install_yaml.contains("Install Dependencies"),
        "Third step should be Install Dependencies"
    );

    // Fourth step should be Run Tests
    let test_yaml = serde_yaml::to_string(&circleci_job.steps[3].raw).unwrap();
    assert!(
        test_yaml.contains("Run Tests"),
        "Fourth step should be Run Tests"
    );
}
