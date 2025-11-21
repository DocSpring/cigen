use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;

use super::step::{Artifact, Step};

/// Package requirement for a job
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackageSpec {
    /// Logical package name (e.g., ruby, node)
    pub name: String,

    /// Optional package manager override
    #[serde(default)]
    pub manager: Option<String>,

    /// Directory to install or use the package manager from
    #[serde(default)]
    pub path: Option<String>,

    /// Explicit version requirement
    #[serde(default)]
    pub version: Option<String>,

    /// Preserve any additional keys for provider-specific behaviour
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

impl PackageSpec {
    pub fn from_name(name: String) -> Self {
        Self {
            name,
            manager: None,
            path: None,
            version: None,
            extra: HashMap::new(),
        }
    }
}

/// Job definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Job {
    /// Job dependencies
    #[serde(default)]
    pub needs: Vec<String>,

    /// Matrix build configuration
    #[serde(default)]
    pub matrix: Option<JobMatrix>,

    /// Package managers to use
    #[serde(default, deserialize_with = "deserialize_packages")]
    pub packages: Vec<PackageSpec>,

    /// Service containers
    #[serde(default)]
    pub services: Vec<String>,

    /// Environment variables
    #[serde(default, alias = "env")]
    pub environment: HashMap<String, String>,

    /// Checkout configuration overrides (applied to the auto checkout step)
    #[serde(default)]
    pub checkout: Option<HashMap<String, Value>>,

    /// Job steps
    #[serde(default)]
    pub steps: Vec<Step>,

    /// Source files that trigger this job (for skip logic)
    #[serde(
        default,
        deserialize_with = "deserialize_source_files",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub source_files: Vec<String>,

    /// Skip conditions
    #[serde(default)]
    pub skip_if: Option<SkipConditions>,

    /// Trigger conditions
    #[serde(default)]
    pub trigger: Option<JobTrigger>,

    /// Docker image or runner class (e.g. "rust:latest", "ubuntu-latest")
    #[serde(default = "default_image")]
    pub image: String,

    /// Runner class (deprecated, use image instead)
    #[serde(default)]
    pub runner: Option<String>,

    /// Artifacts to store
    #[serde(default)]
    pub artifacts: Vec<Artifact>,

    /// Additional unspecified job fields to preserve pass-through metadata
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,

    /// Workflow this job belongs to (set by loader)
    #[serde(default, skip_serializing)]
    pub workflow: Option<String>,

    /// Stage this job belongs to (set by loader from directory structure)
    #[serde(default, skip_serializing)]
    pub stage: Option<String>,
}

fn deserialize_packages<'de, D>(deserializer: D) -> Result<Vec<PackageSpec>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(name)) => Ok(vec![PackageSpec::from_name(name)]),
        Some(Value::Sequence(seq)) => {
            let mut specs = Vec::new();
            for item in seq {
                specs.push(value_to_package(item).map_err(de::Error::custom)?);
            }
            Ok(specs)
        }
        Some(Value::Mapping(map)) => value_to_package(Value::Mapping(map))
            .map(|spec| vec![spec])
            .map_err(de::Error::custom),
        Some(other) => Err(de::Error::custom(format!(
            "Unsupported packages format: {other:?}"
        ))),
    }
}

fn value_to_package(value: Value) -> Result<PackageSpec, serde_yaml::Error> {
    match value {
        Value::String(name) => Ok(PackageSpec::from_name(name)),
        other => serde_yaml::from_value(other),
    }
}

fn deserialize_source_files<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SourceFilesInput {
        Single(String),
        Multiple(Vec<String>),
    }

    let value = Option::<SourceFilesInput>::deserialize(deserializer)?;
    Ok(match value {
        Some(SourceFilesInput::Single(pattern)) => vec![pattern],
        Some(SourceFilesInput::Multiple(patterns)) => patterns,
        None => Vec::new(),
    })
}

fn default_image() -> String {
    "ubuntu-latest".to_string()
}

/// Matrix configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum JobMatrix {
    /// Standard Cartesian dimensions: { key: [v1, v2] }
    Dimensions(HashMap<String, Vec<String>>),
    /// Explicit rows: [ { key: v1 }, { key: v2 } ]
    Explicit(Vec<HashMap<String, String>>),
}

// Deprecated: MatrixDimension was used inside HashMap
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MatrixDimension {
    List(Vec<String>),
}

/// Skip conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkipConditions {
    /// Skip if these file patterns are unchanged
    #[serde(default)]
    pub paths_unmodified: Vec<String>,

    /// Skip if these environment variables are set
    #[serde(default)]
    pub env: Vec<String>,

    /// Skip on these branch patterns
    #[serde(default)]
    pub branch: Vec<String>,
}

/// Job trigger conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum JobTrigger {
    /// Simple trigger type
    Simple(SimpleTrigger),

    /// Complex trigger with patterns
    Complex(ComplexTrigger),
}

/// Simple trigger types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SimpleTrigger {
    /// Manual workflow dispatch
    Manual,
    /// Scheduled (cron)
    Scheduled,
}

/// Complex trigger with filters
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComplexTrigger {
    /// Git tag pattern
    #[serde(default)]
    pub tags: Option<String>,

    /// Branch pattern
    #[serde(default)]
    pub branches: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_job() {
        let yaml = r#"
packages:
  - ruby
steps:
  - run: bundle exec rspec
"#;

        let job: Job = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(job.packages.len(), 1);
        assert_eq!(job.packages[0].name, "ruby");
        assert_eq!(job.steps.len(), 1);
    }

    #[test]
    fn test_packages_string() {
        let yaml = r#"
packages: ruby
steps: []
"#;

        let job: Job = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(job.packages.len(), 1);
        assert_eq!(job.packages[0].name, "ruby");
    }

    #[test]
    fn test_packages_objects() {
        let yaml = r#"
packages:
  - name: node
    path: docs
  - ruby
"#;

        let job: Job = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(job.packages.len(), 2);
        assert_eq!(job.packages[0].name, "node");
        assert_eq!(job.packages[0].path.as_deref(), Some("docs"));
        assert_eq!(job.packages[1].name, "ruby");
    }

    #[test]
    fn test_job_with_matrix() {
        let yaml = r#"
matrix:
  ruby:
    - "3.2"
    - "3.3"
  arch:
    - amd64
    - arm64
"#;

        let job: Job = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(job.matrix.len(), 2);

        if let Some(MatrixDimension::List(values)) = job.matrix.get("ruby") {
            assert_eq!(values, &vec!["3.2", "3.3"]);
        } else {
            panic!("Expected ruby matrix dimension");
        }
    }

    #[test]
    fn test_job_with_services() {
        let yaml = r#"
services:
  - postgres:15
  - redis:7
"#;

        let job: Job = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(job.services, vec!["postgres:15", "redis:7"]);
    }

    #[test]
    fn test_job_with_skip_conditions() {
        let yaml = r#"
skip_if:
  paths_unmodified:
    - app/**
    - spec/**
  env:
    - SKIP_TESTS
"#;

        let job: Job = serde_yaml::from_str(yaml).unwrap();
        let skip = job.skip_if.unwrap();
        assert_eq!(skip.paths_unmodified, vec!["app/**", "spec/**"]);
        assert_eq!(skip.env, vec!["SKIP_TESTS"]);
    }

    #[test]
    fn test_job_with_simple_trigger() {
        let yaml = "trigger: manual";

        let job: Job = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(job.trigger, Some(JobTrigger::Simple(SimpleTrigger::Manual)));
    }

    #[test]
    fn test_job_with_complex_trigger() {
        let yaml = r#"
trigger:
  tags: v*
"#;

        let job: Job = serde_yaml::from_str(yaml).unwrap();
        match job.trigger {
            Some(JobTrigger::Complex(trigger)) => {
                assert_eq!(trigger.tags.as_deref(), Some("v*"));
            }
            other => panic!("Unexpected trigger {:?}", other),
        }
    }
}
