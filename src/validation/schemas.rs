use jsonschema::{Retrieve, Uri};
use serde_json::Value;
use std::str::FromStr;

// Embed schemas at compile time
const CONFIG_SCHEMA: &str = include_str!("../../schemas/v1/config-schema.json");
const CONFIG_BASE_SCHEMA: &str = include_str!("../../schemas/v1/config-base-schema.json");
const WORKFLOW_CONFIG_SCHEMA: &str = include_str!("../../schemas/v1/workflow-config-schema.json");
const JOB_SCHEMA: &str = include_str!("../../schemas/v1/job-schema.json");
const COMMAND_SCHEMA: &str = include_str!("../../schemas/v1/command-schema.json");
const DEFINITIONS_SCHEMA: &str = include_str!("../../schemas/v1/definitions.json");
const DRAFT_07_SCHEMA: &str = include_str!("../../schemas/vendor/draft-07-schema.json");

// Custom retriever for embedded schemas
pub struct SchemaRetriever;

impl Retrieve for SchemaRetriever {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        match uri.as_str() {
            "./config-base-schema.json" => Ok(serde_json::from_str(CONFIG_BASE_SCHEMA)?),
            "https://cigen.dev/schemas/v1/config-base-schema.json" => {
                Ok(serde_json::from_str(CONFIG_BASE_SCHEMA)?)
            }
            "./workflow-config-schema.json" => Ok(serde_json::from_str(WORKFLOW_CONFIG_SCHEMA)?),
            "https://cigen.dev/schemas/v1/workflow-config-schema.json" => {
                Ok(serde_json::from_str(WORKFLOW_CONFIG_SCHEMA)?)
            }
            "./job-schema.json" => Ok(serde_json::from_str(JOB_SCHEMA)?),
            "https://cigen.dev/schemas/v1/job-schema.json" => Ok(serde_json::from_str(JOB_SCHEMA)?),
            "./command-schema.json" => Ok(serde_json::from_str(COMMAND_SCHEMA)?),
            "https://cigen.dev/schemas/v1/command-schema.json" => {
                Ok(serde_json::from_str(COMMAND_SCHEMA)?)
            }
            "./definitions.json" => Ok(serde_json::from_str(DEFINITIONS_SCHEMA)?),
            "https://cigen.dev/schemas/v1/definitions.json" => {
                Ok(serde_json::from_str(DEFINITIONS_SCHEMA)?)
            }
            // Handle the relative reference with fragment
            "./definitions.json#/definitions/configProperties" => {
                Ok(serde_json::from_str(DEFINITIONS_SCHEMA)?)
            }
            // Handle json-schema URI scheme
            "json-schema:///definitions.json" => Ok(serde_json::from_str(DEFINITIONS_SCHEMA)?),
            "https://json-schema.org/draft-07/schema"
            | "https://json-schema.org/draft-07/schema#"
            | "http://json-schema.org/draft-07/schema"
            | "http://json-schema.org/draft-07/schema#" => {
                Ok(serde_json::from_str(DRAFT_07_SCHEMA)?)
            }
            // Handle fragment references by stripping the fragment part
            uri_str if uri_str.contains('#') => {
                let base_uri = uri_str.split('#').next().unwrap_or("");
                self.retrieve(
                    &Uri::from_str(base_uri).map_err(|e| format!("Failed to parse URI: {e}"))?,
                )
            }
            _ => Err(format!("Unknown schema URI: {uri}").into()),
        }
    }
}

// Schema getters
pub fn get_config_schema() -> Result<Value, serde_json::Error> {
    serde_json::from_str(CONFIG_SCHEMA)
}

pub fn get_config_base_schema() -> Result<Value, serde_json::Error> {
    serde_json::from_str(CONFIG_BASE_SCHEMA)
}

pub fn get_job_schema() -> Result<Value, serde_json::Error> {
    serde_json::from_str(JOB_SCHEMA)
}

pub fn get_command_schema() -> Result<Value, serde_json::Error> {
    serde_json::from_str(COMMAND_SCHEMA)
}

pub fn get_workflow_config_schema() -> Result<Value, serde_json::Error> {
    serde_json::from_str(WORKFLOW_CONFIG_SCHEMA)
}
