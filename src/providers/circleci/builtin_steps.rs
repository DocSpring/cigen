use once_cell::sync::Lazy;
use std::collections::HashSet;

/// List of built-in CircleCI step types
/// Based on CircleCI Configuration Reference documentation
pub static BUILTIN_STEPS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        // Core workflow steps
        "checkout",
        "run",
        // Caching steps
        "save_cache",
        "restore_cache",
        // Artifact and test result storage
        "store_artifacts",
        "store_test_results",
        // Workspace persistence
        "persist_to_workspace",
        "attach_workspace",
        // SSH and Docker setup
        "add_ssh_keys",
        "setup_remote_docker",
        // Conditional execution
        "when",
        "unless",
        // Deprecated but still supported
        "deploy",
    ])
});

/// Check if a step name is a built-in CircleCI step
pub fn is_builtin_step(step_name: &str) -> bool {
    BUILTIN_STEPS.contains(step_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_steps() {
        assert!(is_builtin_step("checkout"));
        assert!(is_builtin_step("run"));
        assert!(is_builtin_step("save_cache"));
        assert!(!is_builtin_step("custom_step"));
        assert!(!is_builtin_step("my_command"));
    }
}
