use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;

use super::step::Step;

/// Definition of a reusable command (CircleCI style)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandDefinition {
    /// Optional description for the command
    #[serde(default)]
    pub description: Option<String>,

    /// Parameters accepted by the command
    #[serde(default)]
    pub parameters: HashMap<String, CommandParameter>,

    /// Steps executed by the command
    #[serde(default)]
    pub steps: Vec<Step>,

    /// Preserve any additional metadata
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

/// Parameter definition inside a command
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandParameter {
    /// Declared parameter type (string, boolean, integer)
    #[serde(default, rename = "type")]
    pub parameter_type: Option<String>,

    /// Description shown in CircleCI UI
    #[serde(default)]
    pub description: Option<String>,

    /// Default value for the parameter
    #[serde(default)]
    pub default: Option<Value>,

    /// Preserve any additional metadata or custom fields
    #[serde(default, flatten)]
    pub extra: Mapping,
}
