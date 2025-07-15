use crate::models::{Command, Config, Job};
use anyhow::{Context, Result};

pub struct V1ConfigLoader;
pub struct V1JobLoader;
pub struct V1CommandLoader;

impl V1ConfigLoader {
    pub fn load(content: &str) -> Result<Config> {
        serde_yaml::from_str(content).with_context(|| "Failed to parse v1 config from YAML")
    }
}

impl V1JobLoader {
    pub fn load(content: &str) -> Result<Job> {
        serde_yaml::from_str(content).with_context(|| "Failed to parse v1 job from YAML")
    }
}

impl V1CommandLoader {
    pub fn load(content: &str) -> Result<Command> {
        serde_yaml::from_str(content).with_context(|| "Failed to parse v1 command from YAML")
    }
}
