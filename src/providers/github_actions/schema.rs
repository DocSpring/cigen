use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// GitHub Actions workflow structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<WorkflowTrigger>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,

    pub jobs: HashMap<String, Job>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<Concurrency>,
}

/// Workflow trigger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkflowTrigger {
    Simple(Vec<String>),
    Detailed(HashMap<String, TriggerConfig>),
}

/// Trigger configuration for specific events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branches: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,
}

/// Concurrency configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concurrency {
    pub group: String,

    #[serde(skip_serializing_if = "Option::is_none", rename = "cancel-in-progress")]
    pub cancel_in_progress: Option<bool>,
}

/// GitHub Actions job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "runs-on")]
    pub runs_on: Option<RunsOn>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "if")]
    pub condition: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<Strategy>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<Step>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<HashMap<String, Service>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<Container>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "timeout-minutes")]
    pub timeout_minutes: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<Environment>,
}

/// Runner specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RunsOn {
    Single(String),
    Multiple(Vec<String>),
    Detailed(RunsOnDetailed),
}

/// Detailed runner specification with groups and labels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunsOnDetailed {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

/// Matrix strategy for parameterized builds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub matrix: HashMap<String, Vec<serde_json::Value>>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "fail-fast")]
    pub fail_fast: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "max-parallel")]
    pub max_parallel: Option<u32>,
}

/// Service container configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub image: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<ContainerCredentials>,
}

/// Container configuration for job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub image: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<ContainerCredentials>,
}

/// Container registry credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerCredentials {
    pub username: String,
    pub password: String,
}

/// Workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uses: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub with: Option<HashMap<String, serde_json::Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "if")]
    pub condition: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "working-directory")]
    pub working_directory: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "continue-on-error")]
    pub continue_on_error: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "timeout-minutes")]
    pub timeout_minutes: Option<u32>,
}

/// Permissions configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Permissions {
    /// Simple read/write permissions
    Simple(PermissionLevel),
    /// Detailed per-scope permissions
    Detailed(HashMap<String, PermissionLevel>),
}

/// Permission level for a scope
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionLevel {
    Read,
    Write,
    None,
}

/// Environment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Environment {
    /// Simple environment name
    Simple(String),
    /// Detailed environment with URL
    Detailed {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
}
