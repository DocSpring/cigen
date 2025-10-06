/// End-to-end test of the orchestrator workflow
use cigen::orchestrator::WorkflowOrchestrator;
use cigen::schema::CigenConfig;
use std::path::PathBuf;

#[tokio::test]
async fn test_orchestrator_with_minimal_config() {
    // Create a minimal config
    let yaml = r#"
jobs:
  test:
    packages:
      - ruby
    steps:
      - run: bundle exec rspec
"#;

    let config = CigenConfig::from_yaml(yaml).expect("Failed to parse config");

    // Create orchestrator pointing to the built plugin
    let plugin_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug");

    // Check if the plugin binary exists
    let plugin_path = plugin_dir.join("cigen-provider-github");
    if !plugin_path.exists() {
        eprintln!(
            "Plugin binary not found at: {}. Build with 'cargo build' first.",
            plugin_path.display()
        );
        return; // Skip test if plugin not built
    }

    let mut orchestrator = WorkflowOrchestrator::new(plugin_dir);

    // Execute the workflow with github provider
    let mut test_config = config.clone();
    test_config.providers = vec!["github".to_string()];

    let result = orchestrator
        .execute(test_config)
        .await
        .expect("Failed to execute workflow");

    // Verify we got output files
    assert!(
        !result.files.is_empty(),
        "Expected at least one output file"
    );
    assert!(
        result.files.contains_key(".github/workflows/ci.yml"),
        "Expected GitHub Actions workflow file"
    );

    // Verify the content looks reasonable
    let workflow = &result.files[".github/workflows/ci.yml"];
    assert!(workflow.contains("name: CI"), "Expected workflow name");
    assert!(workflow.contains("jobs:"), "Expected jobs section");
    assert!(
        workflow.contains("bundle exec rspec"),
        "Expected test command"
    );

    println!("Generated workflow:\n{}", workflow);
}

#[tokio::test]
async fn test_orchestrator_with_matrix_config() {
    let yaml = r#"
jobs:
  test:
    matrix:
      ruby:
        - "3.2"
        - "3.3"
    packages:
      - ruby
    steps:
      - run: bundle exec rspec
"#;

    let config = CigenConfig::from_yaml(yaml).expect("Failed to parse config");

    let plugin_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug");
    let plugin_path = plugin_dir.join("cigen-provider-github");

    if !plugin_path.exists() {
        eprintln!("Plugin binary not found. Build with 'cargo build' first. Skipping test.");
        return;
    }

    let mut orchestrator = WorkflowOrchestrator::new(plugin_dir);

    let mut test_config = config.clone();
    test_config.providers = vec!["github".to_string()];

    let result = orchestrator
        .execute(test_config)
        .await
        .expect("Failed to execute workflow");

    let workflow = &result.files[".github/workflows/ci.yml"];

    // The DAG expands the matrix, but for now the plugin generates based on the original schema
    // So we should still see the test job
    assert!(workflow.contains("jobs:"), "Expected jobs section");

    println!("Generated workflow with matrix:\n{}", workflow);
}
