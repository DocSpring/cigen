use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub description: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, Parameter>>,

    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    #[serde(rename = "type")]
    pub param_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#enum: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Step {
    /// Simple run step with name, run command, and optional when condition
    Simple {
        name: String,
        run: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        when: Option<String>,
    },
    /// Reference to another command
    CommandRef(String),
    /// Raw CircleCI step (orb commands, etc)
    Raw(serde_yaml::Value),
}

impl Command {
    /// Load command from YAML content
    pub fn from_yaml(content: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(content)
    }

    /// Get all parameter names defined in this command
    pub fn parameter_names(&self) -> Vec<&String> {
        self.parameters
            .as_ref()
            .map(|params| params.keys().collect())
            .unwrap_or_default()
    }
}
