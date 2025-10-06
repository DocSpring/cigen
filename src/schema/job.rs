use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::step::{Artifact, Step};

/// Job definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Job {
    /// Job dependencies
    #[serde(default)]
    pub needs: Vec<String>,

    /// Matrix build dimensions
    #[serde(default)]
    pub matrix: HashMap<String, MatrixDimension>,

    /// Package managers to use
    #[serde(default)]
    pub packages: Vec<String>,

    /// Service containers
    #[serde(default)]
    pub services: Vec<String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Job steps
    #[serde(default)]
    pub steps: Vec<Step>,

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
}

fn default_image() -> String {
    "ubuntu-latest".to_string()
}

/// Matrix dimension values
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MatrixDimension {
    /// Simple list of values
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
        assert_eq!(job.packages, vec!["ruby"]);
        assert_eq!(job.steps.len(), 1);
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
        if let Some(JobTrigger::Complex(trigger)) = job.trigger {
            assert_eq!(trigger.tags, Some("v*".to_string()));
        } else {
            panic!("Expected complex trigger");
        }
    }
}
