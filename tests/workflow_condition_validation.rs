use cigen::schema::CigenConfig;

fn base_config_head() -> &'static str {
    r#"
providers:
  - github
jobs:
  build:
    steps:
      - run: echo "hello"
"#
}

#[test]
fn github_parameter_conditions_are_rejected() {
    let yaml = format!(
        "{}\nworkflows:\n  main:\n    run_when:\n      - provider: github\n        parameter: run_docs\n",
        base_config_head()
    );

    assert!(
        CigenConfig::from_yaml(&yaml).is_err(),
        "parameter conditions should be rejected for GitHub"
    );
}

#[test]
fn github_expression_conditions_are_supported() {
    let yaml = format!(
        "{}\nworkflows:\n  main:\n    run_when:\n      - provider: github\n        expression: github.event_name == 'workflow_dispatch'\n",
        base_config_head()
    );

    assert!(
        CigenConfig::from_yaml(&yaml).is_ok(),
        "expected expression condition for GitHub to be accepted"
    );
}

#[test]
fn github_env_conditions_are_rejected() {
    let yaml = format!(
        "{}\nworkflows:\n  main:\n    run_when:\n      - provider: github\n        env: FEATURE_FLAG\n",
        base_config_head()
    );

    assert!(
        CigenConfig::from_yaml(&yaml).is_err(),
        "env-based conditions should be rejected for GitHub"
    );
}
