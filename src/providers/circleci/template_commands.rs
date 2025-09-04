use once_cell::sync::Lazy;
use serde_yaml::Value;
use std::collections::HashMap;

/// Built-in template commands that cigen provides
pub static TEMPLATE_COMMANDS: Lazy<HashMap<String, Value>> = Lazy::new(|| {
    let mut commands = HashMap::new();

    // continue_circleci_pipeline command
    let continue_pipeline = serde_yaml::from_str(
        r#"
description: "Continue a CircleCI pipeline with a dynamic configuration"
parameters:
  config_path:
    type: string
    description: "Path to the configuration file to continue with"
    default: ".circleci/dynamic_config.yml"
steps:
  - run:
      name: Continue Pipeline
      environment:
        CONFIG_PATH: << parameters.config_path >>
      command: |
        # Continue with pipeline
        if [ -z "${CIRCLE_CONTINUATION_KEY}" ]; then
            echo "CIRCLE_CONTINUATION_KEY is required. Make sure setup workflows are enabled."
            exit 1
        fi

        if [ -z "${CONFIG_PATH}" ]; then
            echo "CONFIG_PATH is required."
            exit 1
        fi

        # Using --rawfile to read config from file
        jq -n \
          --arg continuation "$CIRCLE_CONTINUATION_KEY" \
          --rawfile config "$CONFIG_PATH" \
          '{"continuation-key": $continuation, "configuration": $config}' \
          > /tmp/continuation.json

        echo "Next CircleCI config:"
        cat /tmp/continuation.json

        [[ $(curl \
                -o /dev/stderr \
                -w '%{http_code}' \
                -XPOST \
                -H "Content-Type: application/json" \
                -H "Accept: application/json"  \
                --data "@/tmp/continuation.json" \
                "https://circleci.com/api/v2/pipeline/continue") \
          -eq 200 ]]
"#,
    )
    .expect("Failed to parse continue_circleci_pipeline template");

    commands.insert("continue_circleci_pipeline".to_string(), continue_pipeline);

    // shallow_checkout command - vendored from git-shallow-clone-orb (MIT license)
    let shallow_checkout = serde_yaml::from_str(include_str!("templates/shallow_checkout.yml"))
        .expect("Failed to parse shallow_checkout template");
    commands.insert("cigen_shallow_checkout".to_string(), shallow_checkout);

    // Add more template commands here in the future
    // For example:
    // - setup_remote_docker_with_cache
    // - etc.

    commands
});

/// Check if a command name is a template command
pub fn is_template_command(command_name: &str) -> bool {
    TEMPLATE_COMMANDS.contains_key(command_name)
}

/// Get a template command definition
pub fn get_template_command(command_name: &str) -> Option<&Value> {
    TEMPLATE_COMMANDS.get(command_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_commands_loaded() {
        assert!(is_template_command("continue_circleci_pipeline"));
        assert!(is_template_command("cigen_shallow_checkout"));
        assert!(!is_template_command("unknown_command"));
    }

    #[test]
    fn test_get_template_command() {
        let cmd = get_template_command("continue_circleci_pipeline");
        assert!(cmd.is_some());

        let cmd_value = cmd.unwrap();
        assert!(cmd_value.get("description").is_some());
        assert!(cmd_value.get("parameters").is_some());
        assert!(cmd_value.get("steps").is_some());
    }

    #[test]
    fn test_shallow_checkout_command() {
        let cmd = get_template_command("cigen_shallow_checkout");
        assert!(cmd.is_some());

        let cmd_value = cmd.unwrap();
        assert!(cmd_value.get("description").is_some());
        assert!(cmd_value.get("parameters").is_some());
        assert!(cmd_value.get("steps").is_some());

        // Check that expected parameters exist
        let params = cmd_value.get("parameters").unwrap();
        assert!(params.get("clone_options").is_some());
        assert!(params.get("fetch_options").is_some());
        assert!(params.get("keyscan_github").is_some());
        assert!(params.get("keyscan_gitlab").is_some());
        assert!(params.get("keyscan_bitbucket").is_some());
    }
}
