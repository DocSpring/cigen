pub mod circleci;

use crate::models::{Config, Job};
use miette::Result;
use std::collections::HashMap;
use std::path::Path;

/// Trait for CI provider implementations
pub trait Provider: Send + Sync {
    /// Name of the provider (e.g., "circleci", "github-actions")
    fn name(&self) -> &'static str;

    /// Generate CI configuration for a single workflow
    fn generate_workflow(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
        output_path: &Path,
    ) -> Result<()>;

    /// Generate CI configuration for all workflows
    fn generate_all(
        &self,
        config: &Config,
        workflows: &HashMap<String, HashMap<String, Job>>,
        output_path: &Path,
    ) -> Result<()>;
}

/// Get a provider implementation by name
pub fn get_provider(name: &str) -> Result<Box<dyn Provider>> {
    match name {
        "circleci" => Ok(Box::new(circleci::CircleCIProvider::new())),
        _ => Err(miette::miette!("Unknown provider: {}", name)),
    }
}