mod config;
mod generator;
mod schema;

#[cfg(test)]
mod tests;

use crate::models::{Config, Job};
use crate::providers::Provider;
use miette::Result;
use std::collections::HashMap;
use std::path::Path;

pub struct CircleCIProvider {
    generator: generator::CircleCIGenerator,
}

impl CircleCIProvider {
    pub fn new() -> Self {
        Self {
            generator: generator::CircleCIGenerator::new(),
        }
    }
}

impl Default for CircleCIProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for CircleCIProvider {
    fn name(&self) -> &'static str {
        "circleci"
    }

    fn default_output_path(&self) -> &'static str {
        ".circleci"
    }

    fn generate_workflow(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
        output_path: &Path,
    ) -> Result<()> {
        self.generator
            .generate_workflow(config, workflow_name, jobs, output_path)
    }

    fn generate_all(
        &self,
        config: &Config,
        workflows: &HashMap<String, HashMap<String, Job>>,
        output_path: &Path,
    ) -> Result<()> {
        self.generator.generate_all(config, workflows, output_path)
    }
}
