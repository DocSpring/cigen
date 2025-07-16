use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashSet;

/// The full CircleCI public schema loaded at compile time
pub static CIRCLECI_SCHEMA: Lazy<Value> = Lazy::new(|| {
    serde_json::from_str(include_str!(
        "../../../schemas/vendor/circleci-publicschema.json"
    ))
    .expect("Failed to parse CircleCI schema")
});

/// Set of built-in CircleCI step names extracted from the schema
pub static BUILTIN_STEPS: Lazy<HashSet<String>> = Lazy::new(|| {
    let mut steps = HashSet::new();

    // Extract built-in steps from the schema
    if let Some(builtin_steps) = CIRCLECI_SCHEMA
        .get("definitions")
        .and_then(|v| v.get("builtinSteps"))
        .and_then(|v| v.get("documentation"))
        .and_then(|v| v.as_object())
    {
        for step_name in builtin_steps.keys() {
            steps.insert(step_name.to_string());
        }
    }

    steps
});

/// Check if a step name is a built-in CircleCI step
pub fn is_builtin_step(step_name: &str) -> bool {
    BUILTIN_STEPS.contains(step_name)
}

/// Validate a CircleCI configuration against the schema
#[allow(dead_code)]
pub fn validate_config(_config: &Value) -> Result<(), Vec<String>> {
    // TODO: Implement full schema validation using jsonschema crate
    // For now, we'll just return Ok
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_loads() {
        // Ensure the schema loads properly
        assert!(CIRCLECI_SCHEMA.is_object());
    }

    #[test]
    fn test_builtin_steps_extracted() {
        // Debug: print what steps we actually extracted
        println!("Extracted steps: {:?}", &*BUILTIN_STEPS);

        // Check that we extracted the built-in steps
        assert!(is_builtin_step("checkout"));
        assert!(is_builtin_step("run"));
        assert!(is_builtin_step("save_cache"));
        assert!(is_builtin_step("restore_cache"));
        assert!(is_builtin_step("store_artifacts"));
        assert!(is_builtin_step("store_test_results"));
        assert!(is_builtin_step("persist_to_workspace"));
        assert!(is_builtin_step("attach_workspace"));
        assert!(is_builtin_step("add_ssh_keys"));
        assert!(is_builtin_step("setup_remote_docker"));
        assert!(is_builtin_step("when"));
        assert!(is_builtin_step("unless"));
        assert!(is_builtin_step("deploy"));

        // Check that non-builtin steps return false
        assert!(!is_builtin_step("custom_step"));
        assert!(!is_builtin_step("my_command"));
        assert!(!is_builtin_step("continue_circleci_pipeline"));
    }
}
