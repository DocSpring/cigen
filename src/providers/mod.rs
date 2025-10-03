pub mod circleci;
pub mod github_actions;

use crate::models::{Command, Config, Job};
use miette::Result;
use std::collections::HashMap;
use std::path::Path;

/// Trait for CI provider implementations
pub trait Provider: Send + Sync {
    /// Name of the provider (e.g., "circleci", "github-actions")
    fn name(&self) -> &'static str;

    /// Default output path for this provider
    fn default_output_path(&self) -> &'static str;

    /// Generate CI configuration for a single workflow
    fn generate_workflow(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
        commands: &HashMap<String, Command>,
        output_path: &Path,
    ) -> Result<()>;

    /// Generate CI configuration for all workflows
    fn generate_all(
        &self,
        config: &Config,
        workflows: &HashMap<String, HashMap<String, Job>>,
        commands: &HashMap<String, Command>,
        output_path: &Path,
    ) -> Result<()>;
}

/// Get a provider implementation by name
pub fn get_provider(name: &str) -> Result<Box<dyn Provider>> {
    match name {
        "circleci" => Ok(Box::new(circleci::CircleCIProvider::new())),
        "github-actions" => Ok(Box::new(github_actions::GitHubActionsProvider::new())),
        _ => Err(miette::miette!("Unknown provider: {}", name)),
    }
}
