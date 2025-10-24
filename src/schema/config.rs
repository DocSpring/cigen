use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;

use super::command::CommandDefinition;
use super::job::Job;
use super::workflow::{WorkflowConditionKind, WorkflowConfig};

/// Main cigen.yml configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CigenConfig {
    /// Project metadata
    #[serde(default)]
    pub project: Option<ProjectConfig>,

    /// Providers to generate configs for
    #[serde(default)]
    pub providers: Vec<String>,

    /// Global packages available to all jobs
    #[serde(default)]
    pub packages: Vec<String>,

    /// Named source file groups (for skip logic)
    #[serde(default)]
    pub source_file_groups: HashMap<String, Vec<String>>,

    /// Job definitions (required)
    pub jobs: HashMap<String, Job>,

    /// Reusable command definitions
    #[serde(default)]
    pub commands: HashMap<String, CommandDefinition>,

    /// Cache definitions (optional, overrides defaults)
    #[serde(default)]
    pub caches: HashMap<String, CacheDefinition>,

    /// Runner definitions
    #[serde(default)]
    pub runners: HashMap<String, RunnerDefinition>,

    /// Provider-specific configuration
    #[serde(default)]
    pub provider_config: HashMap<String, serde_yaml::Value>,

    /// Workflow metadata (triggers, permissions, etc.) keyed by workflow id
    #[serde(default)]
    pub workflows: HashMap<String, WorkflowConfig>,

    /// Raw merged configuration (for provider-specific logic)
    #[serde(skip)]
    pub raw: Mapping,
}

/// Project configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectConfig {
    /// Human-readable project name
    pub name: String,

    /// Project type for special handling
    #[serde(default)]
    pub r#type: Option<ProjectType>,

    /// Default runner for all jobs
    #[serde(default)]
    pub default_runner: Option<String>,
}

/// Project type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    /// Turborepo monorepo
    Turborepo,
    /// Default/standard project
    Default,
}

/// Cache definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheDefinition {
    /// Paths to cache
    pub paths: Vec<String>,

    /// Components for cache key
    pub key_parts: Vec<String>,

    /// Cache backend
    #[serde(default = "default_cache_backend")]
    pub backend: CacheBackend,
}

fn default_cache_backend() -> CacheBackend {
    CacheBackend::Native
}

/// Cache backend
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CacheBackend {
    /// Provider's native caching
    Native,
    /// Redis-based cache
    Redis,
    /// S3-compatible storage
    S3,
}

/// Runner definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunnerDefinition {
    /// Provider-specific configuration
    pub provider_config: HashMap<String, serde_yaml::Value>,
}

impl CigenConfig {
    /// Load configuration from YAML string
    pub fn from_yaml(yaml: &str) -> anyhow::Result<Self> {
        let mut config: CigenConfig = serde_yaml::from_str(yaml)?;
        config.raw = extract_mapping(yaml)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate at least one job is defined
        if self.jobs.is_empty() {
            anyhow::bail!("Configuration must define at least one job");
        }

        // Validate job references in needs
        for (job_id, job) in &self.jobs {
            for needed_job in &job.needs {
                if !self.jobs.contains_key(needed_job) {
                    anyhow::bail!(
                        "Job '{}' references unknown job '{}' in needs",
                        job_id,
                        needed_job
                    );
                }
            }

            // Check for self-reference
            if job.needs.contains(job_id) {
                anyhow::bail!("Job '{}' cannot depend on itself", job_id);
            }
        }

        let providers = self.get_providers();
        for (workflow_id, workflow) in &self.workflows {
            for condition in &workflow.run_when {
                condition.validate().with_context(|| {
                    format!(
                        "Invalid condition in workflow '{}': {:?}",
                        workflow_id, condition
                    )
                })?;
                if matches!(condition.kind(), Some(WorkflowConditionKind::Expression))
                    && condition
                        .expression
                        .as_ref()
                        .map(|s| s.trim().is_empty())
                        .unwrap_or(true)
                {
                    anyhow::bail!(
                        "Workflow '{}' has an expression condition with an empty expression",
                        workflow_id
                    );
                }

                let target_providers: Vec<&str> =
                    if let Some(provider) = condition.provider.as_deref() {
                        vec![provider]
                    } else {
                        providers.clone()
                    };

                for provider in target_providers {
                    if !provider_supports_condition(provider, condition.kind()) {
                        anyhow::bail!(
                            "Workflow '{}' uses a {:?} condition that is not supported by provider '{}'",
                            workflow_id,
                            condition.kind().unwrap_or(WorkflowConditionKind::Parameter),
                            provider
                        );
                    }
                }
            }
        }

        // TODO: Detect circular dependencies
        // TODO: Validate provider names
        // TODO: Validate runner references

        Ok(())
    }

    /// Get all providers to generate for
    pub fn get_providers(&self) -> Vec<&str> {
        if self.providers.is_empty() {
            // Default: all supported providers
            vec!["github", "circleci", "buildkite"]
        } else {
            self.providers.iter().map(|s| s.as_str()).collect()
        }
    }
}

fn provider_supports_condition(provider: &str, kind: Option<WorkflowConditionKind>) -> bool {
    let kind = kind.unwrap_or(WorkflowConditionKind::Parameter);
    match provider {
        "circleci" => matches!(kind, WorkflowConditionKind::Parameter),
        "github" => matches!(kind, WorkflowConditionKind::Expression),
        "buildkite" => {
            // Buildkite currently has no workflow condition support; fail explicitly.
            false
        }
        _ => false,
    }
}

fn extract_mapping(yaml: &str) -> anyhow::Result<Mapping> {
    let value: Value = serde_yaml::from_str(yaml)?;
    match value {
        Value::Mapping(map) => Ok(map),
        other => {
            let mut map = Mapping::new();
            map.insert(Value::String("root".into()), other);
            Ok(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_config() {
        let yaml = r#"
jobs:
  test:
    packages:
      - ruby
    steps:
      - run: bundle exec rspec
"#;

        let config = CigenConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.jobs.len(), 1);
        assert!(config.jobs.contains_key("test"));
    }

    #[test]
    fn test_config_with_project() {
        let yaml = r#"
project:
  name: myapp
  type: turborepo

jobs:
  test:
    packages:
      - node
"#;

        let config = CigenConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.project.as_ref().unwrap().name, "myapp");
        assert_eq!(
            config.project.as_ref().unwrap().r#type,
            Some(ProjectType::Turborepo)
        );
    }

    #[test]
    fn test_config_with_providers() {
        let yaml = r#"
providers:
  - github
  - circleci

jobs:
  test: {}
"#;

        let config = CigenConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.providers, vec!["github", "circleci"]);
    }

    #[test]
    fn test_validation_missing_jobs() {
        let yaml = "jobs: {}";
        let result = CigenConfig::from_yaml(yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one job"));
    }

    #[test]
    fn test_validation_unknown_job_reference() {
        let yaml = r#"
jobs:
  test:
    needs:
      - nonexistent
"#;

        let result = CigenConfig::from_yaml(yaml);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown job 'nonexistent'")
        );
    }

    #[test]
    fn test_validation_self_reference() {
        let yaml = r#"
jobs:
  test:
    needs:
      - test
"#;

        let result = CigenConfig::from_yaml(yaml);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cannot depend on itself")
        );
    }
}
