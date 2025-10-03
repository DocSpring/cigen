use super::GitHubActionsProvider;
use super::schema::{Job as GHJob, RunsOn, Step, Workflow};
use crate::models::{Config, Job, job::Step as CigenStep};
use crate::providers::Provider;
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn test_provider_name() {
    let provider = GitHubActionsProvider::new();
    assert_eq!(provider.name(), "github-actions");
}

#[test]
fn test_default_output_path() {
    let provider = GitHubActionsProvider::new();
    assert_eq!(provider.default_output_path(), ".github/workflows");
}

#[test]
fn test_workflow_serialization() {
    let mut jobs = HashMap::new();
    jobs.insert(
        "test".to_string(),
        GHJob {
            name: Some("Test Job".to_string()),
            runs_on: Some(RunsOn::Single("ubuntu-latest".to_string())),
            needs: None,
            condition: None,
            steps: Some(vec![Step {
                id: None,
                name: Some("Run tests".to_string()),
                uses: None,
                run: Some("cargo test".to_string()),
                with: None,
                env: None,
                condition: None,
                working_directory: None,
                shell: None,
                continue_on_error: None,
                timeout_minutes: None,
            }]),
            env: None,
            strategy: None,
            services: None,
            container: None,
            timeout_minutes: None,
            outputs: None,
        },
    );

    let workflow = Workflow {
        name: "CI".to_string(),
        on: None,
        jobs,
        env: None,
        concurrency: None,
    };

    let yaml = serde_yaml::to_string(&workflow).unwrap();
    assert!(yaml.contains("name: CI"));
    assert!(yaml.contains("jobs:"));
    assert!(yaml.contains("test:"));
    assert!(yaml.contains("runs-on: ubuntu-latest"));
}

#[test]
fn test_basic_workflow_generation() {
    let provider = GitHubActionsProvider::new();
    let temp_dir = TempDir::new().unwrap();

    let config = Config {
        provider: "github-actions".to_string(),
        output_path: None,
        output_filename: None,
        version: None,
        anchors: None,
        caches: None,
        cache_definitions: None,
        version_sources: None,
        architectures: None,
        resource_classes: None,
        docker: None,
        services: None,
        source_file_groups: None,
        vars: None,
        graph: None,
        dynamic: None,
        setup: None,
        parameters: None,
        orbs: None,
        outputs: None,
        docker_images: None,
        docker_build: None,
        package_managers: None,
        workflows: None,
        workflows_meta: None,
        checkout: None,
        setup_options: None,
    };

    // Create a simple run step using YAML
    let step_yaml = serde_yaml::from_str(
        r#"
        name: Run tests
        run: cargo test
        "#,
    )
    .unwrap();

    let mut jobs = HashMap::new();
    jobs.insert(
        "test".to_string(),
        Job {
            image: "rust:latest".to_string(),
            steps: Some(vec![CigenStep(step_yaml)]),
            requires: None,
            checkout: Some(crate::models::config::CheckoutSetting::Bool(true)),
            architectures: None,
            resource_class: None,
            source_files: None,
            source_submodules: None,
            parallelism: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            job_type: None,
        },
    );

    provider
        .generate_workflow(&config, "ci", &jobs, &HashMap::new(), temp_dir.path())
        .unwrap();

    let output_file = temp_dir.path().join("ci.yml");
    assert!(output_file.exists());

    let content = std::fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("name: ci"));
    assert!(content.contains("jobs:"));
    // Should have default triggers
    assert!(content.contains("on:"));
    assert!(content.contains("push:"));
    assert!(content.contains("pull_request:"));
}

#[test]
fn test_matrix_build_generation() {
    let provider = GitHubActionsProvider::new();
    let temp_dir = TempDir::new().unwrap();

    let config = Config {
        provider: "github-actions".to_string(),
        output_path: None,
        output_filename: None,
        version: None,
        anchors: None,
        caches: None,
        cache_definitions: None,
        version_sources: None,
        architectures: None,
        resource_classes: None,
        docker: None,
        services: None,
        source_file_groups: None,
        vars: None,
        graph: None,
        dynamic: None,
        setup: None,
        parameters: None,
        orbs: None,
        outputs: None,
        docker_images: None,
        docker_build: None,
        package_managers: None,
        workflows: None,
        workflows_meta: None,
        checkout: None,
        setup_options: None,
    };

    // Create a job with multiple architectures
    let step_yaml = serde_yaml::from_str(
        r#"
        name: Build
        run: cargo build
        "#,
    )
    .unwrap();

    let mut jobs = HashMap::new();
    jobs.insert(
        "build".to_string(),
        Job {
            image: "rust:latest".to_string(),
            architectures: Some(vec!["amd64".to_string(), "arm64".to_string()]),
            steps: Some(vec![CigenStep(step_yaml)]),
            requires: None,
            checkout: Some(crate::models::config::CheckoutSetting::Bool(true)),
            resource_class: None,
            source_files: None,
            source_submodules: None,
            parallelism: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            job_type: None,
        },
    );

    provider
        .generate_workflow(&config, "ci", &jobs, &HashMap::new(), temp_dir.path())
        .unwrap();

    let output_file = temp_dir.path().join("ci.yml");
    let content = std::fs::read_to_string(&output_file).unwrap();

    // Should have matrix strategy
    assert!(content.contains("strategy:"));
    assert!(content.contains("matrix:"));
    assert!(content.contains("arch:"));
    assert!(content.contains("amd64"));
    assert!(content.contains("arm64"));
}
