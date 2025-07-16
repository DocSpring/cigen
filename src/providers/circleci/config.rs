use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// CircleCI configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIConfig {
    pub version: f32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub orbs: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub executors: Option<HashMap<String, CircleCIExecutor>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<HashMap<String, CircleCICommand>>,

    pub jobs: HashMap<String, CircleCIJob>,

    pub workflows: HashMap<String, CircleCIWorkflow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIExecutor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker: Option<Vec<CircleCIDockerImage>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<CircleCIMachine>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_class: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIDockerImage {
    pub image: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<CircleCIDockerAuth>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIDockerAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIMachine {
    pub image: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker_layer_caching: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCICommand {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, CircleCIParameter>>,

    pub steps: Vec<CircleCIStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CircleCIParameter {
    String {
        #[serde(rename = "type")]
        param_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    Boolean {
        #[serde(rename = "type")]
        param_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIJob {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executor: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker: Option<Vec<CircleCIDockerImage>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<CircleCIMachine>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_class: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallelism: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, CircleCIParameter>>,

    pub steps: Vec<CircleCIStep>,
}

#[derive(Debug, Clone)]
pub struct CircleCIStep {
    // The raw YAML value of the step
    pub raw: serde_yaml::Value,

    // Our detected step type (not serialized)
    pub step_type: Option<String>,
}

impl Serialize for CircleCIStep {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Just serialize the raw value directly
        self.raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CircleCIStep {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialize as raw value and then detect step type
        let raw = serde_yaml::Value::deserialize(deserializer)?;
        Ok(CircleCIStep::new(raw))
    }
}

impl CircleCIStep {
    pub fn new(raw: serde_yaml::Value) -> Self {
        let step_type = Self::detect_step_type(&raw);
        Self { raw, step_type }
    }

    fn detect_step_type(value: &serde_yaml::Value) -> Option<String> {
        match value {
            serde_yaml::Value::String(_) => Some("run".to_string()),
            serde_yaml::Value::Mapping(map) => {
                if map.len() == 1 {
                    if let Some((key, _)) = map.iter().next() {
                        if let Some(key_str) = key.as_str() {
                            return Some(key_str.to_string());
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn is_builtin_step(&self) -> bool {
        if let Some(step_type) = &self.step_type {
            crate::providers::circleci::schema::is_builtin_step(step_type)
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CircleCIRunStep {
    Simple { run: String },
    Detailed { run: CircleCIRunDetails },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIRunDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub command: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_output_timeout: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCISetupRemoteDocker {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker_layer_caching: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCISaveCache {
    pub key: String,
    pub paths: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIRestoreCache {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIStoreArtifacts {
    pub path: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIStoreTestResults {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIPersistToWorkspace {
    pub root: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIAttachWorkspace {
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIAddSSHKeys {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprints: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIWhenStep {
    pub condition: String,
    pub steps: Vec<CircleCIStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIUnlessStep {
    pub condition: String,
    pub steps: Vec<CircleCIStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIWorkflow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<CircleCIWorkflowWhen>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub unless: Option<CircleCIWorkflowWhen>,

    pub jobs: Vec<CircleCIWorkflowJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CircleCIWorkflowWhen {
    Simple(String),
    Complex {
        #[serde(skip_serializing_if = "Option::is_none")]
        and: Option<Vec<String>>,

        #[serde(skip_serializing_if = "Option::is_none")]
        or: Option<Vec<String>>,

        #[serde(skip_serializing_if = "Option::is_none")]
        equal: Option<Vec<serde_yaml::Value>>,

        #[serde(skip_serializing_if = "Option::is_none")]
        not: Option<Box<CircleCIWorkflowWhen>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CircleCIWorkflowJob {
    Simple(String),
    Detailed {
        #[serde(flatten)]
        job: HashMap<String, CircleCIWorkflowJobDetails>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIWorkflowJobDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<CircleCIContext>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<CircleCIFilters>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix: Option<CircleCIMatrix>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub job_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_steps: Option<Vec<CircleCIStep>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_steps: Option<Vec<CircleCIStep>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CircleCIContext {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branches: Option<CircleCIBranchFilter>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<CircleCIBranchFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIBranchFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleCIMatrix {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, Vec<serde_yaml::Value>>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<HashMap<String, serde_yaml::Value>>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

impl Default for CircleCIConfig {
    fn default() -> Self {
        Self {
            version: 2.1,
            setup: None,
            orbs: None,
            executors: None,
            commands: None,
            jobs: HashMap::new(),
            workflows: HashMap::new(),
        }
    }
}
