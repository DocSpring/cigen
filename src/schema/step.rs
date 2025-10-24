use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;

/// Job step
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Step {
    /// Uses a module
    Uses(UsesStep),

    /// Simple run command
    SimpleRun { run: String },

    /// Run step with options (matches { run: { name, command } })
    RunWithOptions { run: RunStepOptions },

    /// CircleCI restore_cache step
    RestoreCache {
        restore_cache: RestoreCacheDefinition,
    },

    /// CircleCI save_cache step
    SaveCache { save_cache: SaveCacheDefinition },

    /// Any other step type - preserved as raw YAML value
    Custom(Value),
}

/// Run step options (for complex run steps)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunStepOptions {
    /// Step name (optional)
    #[serde(default)]
    pub name: Option<String>,

    /// Command to run
    pub command: String,

    /// Environment variables for this step
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Conditional execution
    #[serde(default, rename = "if")]
    pub condition: Option<String>,
}

/// Uses step (module invocation)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsesStep {
    /// Module reference with version (e.g., docker/build@>=1.1)
    pub uses: String,

    /// Module parameters
    #[serde(default)]
    pub with: HashMap<String, serde_yaml::Value>,

    /// Conditional execution
    #[serde(default, rename = "if")]
    pub condition: Option<String>,
}

/// Artifact definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artifact {
    /// Glob pattern for artifact paths
    pub path: String,

    /// Retention period (e.g., "7d", "30d")
    #[serde(default)]
    pub retention: Option<String>,
}

/// restore_cache step options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestoreCacheDefinition {
    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub key: Option<String>,

    #[serde(default)]
    pub keys: Vec<String>,

    #[serde(default)]
    pub restore_keys: Vec<String>,

    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

/// save_cache step options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SaveCacheDefinition {
    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub key: Option<String>,

    #[serde(default)]
    pub paths: Vec<String>,

    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_run_step() {
        let yaml = "run: bundle exec rspec";

        let step: Step = serde_yaml::from_str(yaml).unwrap();
        match step {
            Step::SimpleRun { run } => {
                assert_eq!(run, "bundle exec rspec");
            }
            _ => panic!("Expected SimpleRun"),
        }
    }

    #[test]
    fn test_run_step_with_name() {
        let yaml = r#"
run:
  name: Run tests
  command: bundle exec rspec
"#;

        let step: Step = serde_yaml::from_str(yaml).unwrap();
        match step {
            Step::RunWithOptions { run } => {
                assert_eq!(run.name, Some("Run tests".to_string()));
                assert_eq!(run.command, "bundle exec rspec");
            }
            _ => panic!("Expected RunWithOptions"),
        }
    }

    #[test]
    fn test_uses_step() {
        let yaml = r#"
uses: docker/build@>=1.1
with:
  context: .
  push: false
"#;

        let step: Step = serde_yaml::from_str(yaml).unwrap();
        match step {
            Step::Uses(uses) => {
                assert_eq!(uses.uses, "docker/build@>=1.1");
                assert!(uses.with.contains_key("context"));
            }
            _ => panic!("Expected Uses"),
        }
    }

    #[test]
    fn test_artifact() {
        let yaml = r#"
path: dist/**
retention: 7d
"#;

        let artifact: Artifact = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(artifact.path, "dist/**");
        assert_eq!(artifact.retention, Some("7d".to_string()));
    }

    #[test]
    fn test_restore_cache_step() {
        let yaml = r#"
restore_cache:
  name: restore gems
  keys:
    - gems-v1
"#;

        let step: Step = serde_yaml::from_str(yaml).unwrap();
        match step {
            Step::RestoreCache { restore_cache } => {
                assert_eq!(restore_cache.name.as_deref(), Some("restore gems"));
                assert_eq!(restore_cache.keys, vec!["gems-v1"]);
            }
            _ => panic!("Expected RestoreCache"),
        }
    }

    #[test]
    fn test_custom_step() {
        let yaml = r#"
store_artifacts:
  path: log
"#;

        let step: Step = serde_yaml::from_str(yaml).unwrap();
        match step {
            Step::Custom(value) => {
                assert!(matches!(value, Value::Mapping(_)));
            }
            _ => panic!("Expected Custom"),
        }
    }
}
