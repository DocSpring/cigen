use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StageDefinition {
    pub name: String,
    #[serde(default)]
    pub needs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WorkflowConfig {
    pub dynamic: bool,
    pub output_path: Option<String>,
    pub output_filename: Option<String>,
    pub setup: bool,
    pub checkout: Option<Value>,
    pub run_when: Vec<WorkflowCondition>,
    #[serde(default)]
    pub stages: Vec<StageDefinition>,
    #[serde(default)]
    pub stage_prefix: bool,
    #[serde(default)]
    pub default_stage_prefix: bool,
    #[serde(default = "default_stage_prefix_separator")]
    pub stage_prefix_separator: String,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
    #[serde(skip)]
    pub raw: Value,
}

fn default_stage_prefix_separator() -> String {
    "_".to_string()
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            dynamic: false,
            output_path: None,
            output_filename: None,
            setup: false,
            checkout: None,
            run_when: Vec::new(),
            stages: Vec::new(),
            stage_prefix: false,
            default_stage_prefix: false,
            stage_prefix_separator: default_stage_prefix_separator(),
            extra: HashMap::new(),
            raw: Value::Mapping(Mapping::new()),
        }
    }
}

impl WorkflowConfig {
    pub fn from_value(value: Value) -> Result<Self> {
        let mut config: WorkflowConfig = serde_yaml::from_value(value.clone())?;
        config.raw = value;
        Ok(config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct WorkflowCondition {
    pub provider: Option<String>,
    pub parameter: Option<String>,
    pub variable: Option<String>,
    pub env: Option<String>,
    pub expression: Option<String>,
    pub equals: Option<Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl WorkflowCondition {
    pub fn kind(&self) -> Option<WorkflowConditionKind> {
        if self.parameter.is_some() {
            Some(WorkflowConditionKind::Parameter)
        } else if self.variable.is_some() {
            Some(WorkflowConditionKind::Variable)
        } else if self.env.is_some() {
            Some(WorkflowConditionKind::Env)
        } else if self.expression.is_some() {
            Some(WorkflowConditionKind::Expression)
        } else {
            None
        }
    }

    pub fn key(&self) -> Option<&str> {
        if let Some(param) = self.parameter.as_deref() {
            Some(param)
        } else if let Some(var) = self.variable.as_deref() {
            Some(var)
        } else if let Some(env) = self.env.as_deref() {
            Some(env)
        } else {
            None
        }
    }

    pub fn equals_value(&self) -> Value {
        self.equals.clone().unwrap_or(Value::Bool(true))
    }

    pub fn validate(&self) -> Result<()> {
        if self.kind().is_none() {
            return Err(anyhow!(
                "workflow condition must specify one of: parameter, variable, env, expression"
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowConditionKind {
    Parameter,
    Variable,
    Env,
    Expression,
}
