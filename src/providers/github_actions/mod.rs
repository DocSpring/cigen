mod generator;
pub mod schema;

#[cfg(test)]
mod tests;

use crate::models::{Command, Config, Job};
use crate::providers::Provider;
use miette::Result;
use std::collections::HashMap;
use std::path::Path;

pub struct GitHubActionsProvider {
    pub generator: generator::GitHubActionsGenerator,
}

impl GitHubActionsProvider {
    pub fn new() -> Self {
        Self {
            generator: generator::GitHubActionsGenerator::new(),
        }
    }
}

impl Default for GitHubActionsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for GitHubActionsProvider {
    fn name(&self) -> &'static str {
        "github-actions"
    }

    fn default_output_path(&self) -> &'static str {
        ".github/workflows"
    }

    fn generate_workflow(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
        commands: &HashMap<String, Command>,
        output_path: &Path,
    ) -> Result<()> {
        self.generator
            .generate_workflow(config, workflow_name, jobs, commands, output_path)
    }

    fn generate_all(
        &self,
        config: &Config,
        workflows: &HashMap<String, HashMap<String, Job>>,
        commands: &HashMap<String, Command>,
        output_path: &Path,
    ) -> Result<()> {
        self.generator
            .generate_all(config, workflows, commands, output_path)
    }
}
