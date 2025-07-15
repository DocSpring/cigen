use jsonschema::{Retrieve, Uri};
use serde_json::Value;

// Embed schemas at compile time
const CONFIG_SCHEMA: &str = include_str!("../../schemas/v1/config-schema.json");
const CONFIG_BASE_SCHEMA: &str = include_str!("../../schemas/v1/config-base-schema.json");
const JOB_SCHEMA: &str = include_str!("../../schemas/v1/job-schema.json");
const COMMAND_SCHEMA: &str = include_str!("../../schemas/v1/command-schema.json");
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
            "./job-schema.json" => Ok(serde_json::from_str(JOB_SCHEMA)?),
            "https://cigen.dev/schemas/v1/job-schema.json" => Ok(serde_json::from_str(JOB_SCHEMA)?),
            "./command-schema.json" => Ok(serde_json::from_str(COMMAND_SCHEMA)?),
            "https://cigen.dev/schemas/v1/command-schema.json" => {
                Ok(serde_json::from_str(COMMAND_SCHEMA)?)
            }
            "https://json-schema.org/draft-07/schema"
            | "https://json-schema.org/draft-07/schema#"
            | "http://json-schema.org/draft-07/schema"
            | "http://json-schema.org/draft-07/schema#" => {
                Ok(serde_json::from_str(DRAFT_07_SCHEMA)?)
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
