use super::schema::{Job as GHJob, RunsOn, Step, Workflow};
use crate::models::{Command, Config, Job};
use miette::{IntoDiagnostic, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct GitHubActionsGenerator {}

impl GitHubActionsGenerator {
    pub fn new() -> Self {
        Self {}
    }

    /// Generate a single workflow file
    pub fn generate_workflow(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
        _commands: &HashMap<String, Command>,
        output_path: &Path,
    ) -> Result<()> {
        // Create output directory
        fs::create_dir_all(output_path).into_diagnostic()?;

        // Build workflow structure
        let workflow = self.build_workflow(config, workflow_name, jobs)?;

        // Serialize to YAML
        let yaml = serde_yaml::to_string(&workflow).into_diagnostic()?;

        // Write to file
        let output_file = output_path.join(format!("{workflow_name}.yml"));
        fs::write(&output_file, yaml).into_diagnostic()?;

        println!(
            "Generated GitHub Actions workflow: {}",
            output_file.display()
        );

        Ok(())
    }

    /// Generate all workflows
    pub fn generate_all(
        &self,
        config: &Config,
        workflows: &HashMap<String, HashMap<String, Job>>,
        commands: &HashMap<String, Command>,
        output_path: &Path,
    ) -> Result<()> {
        for (workflow_name, jobs) in workflows {
            self.generate_workflow(config, workflow_name, jobs, commands, output_path)?;
        }

        Ok(())
    }

    /// Build the workflow structure from config
    fn build_workflow(
        &self,
        _config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
    ) -> Result<Workflow> {
        let mut gh_jobs = HashMap::new();

        for (job_name, job) in jobs {
            let gh_job = self.build_job(job)?;
            gh_jobs.insert(job_name.clone(), gh_job);
        }

        Ok(Workflow {
            name: workflow_name.to_string(),
            on: None, // TODO: Parse from workflow config
            jobs: gh_jobs,
            env: None,
            concurrency: None,
        })
    }

    /// Build a GitHub Actions job from a cigen job
    fn build_job(&self, job: &Job) -> Result<GHJob> {
        let steps = self.build_steps(job)?;

        // Convert requires to needs (GitHub Actions only supports AND dependencies)
        let needs = job.requires.as_ref().map(|r| r.to_vec());

        Ok(GHJob {
            name: None, // GitHub Actions infers job name from key
            runs_on: Some(RunsOn::Single("ubuntu-latest".to_string())), // TODO: From config
            needs,
            condition: None, // TODO: Map from job conditions
            steps: Some(steps),
            env: None,      // TODO: Environment variables
            strategy: None, // TODO: Matrix builds
            services: None, // TODO: Service containers
            container: None,
            timeout_minutes: None,
            outputs: None,
        })
    }

    /// Build steps from a cigen job
    fn build_steps(&self, job: &Job) -> Result<Vec<Step>> {
        let mut steps = Vec::new();

        // Add checkout step if needed
        use crate::models::config::CheckoutSetting;
        let should_checkout = match &job.checkout {
            None => true, // Default to true
            Some(CheckoutSetting::Bool(b)) => *b,
            Some(CheckoutSetting::Config(_)) => true, // If detailed checkout config, assume true
        };

        if should_checkout {
            steps.push(Step {
                id: None,
                name: Some("Checkout code".to_string()),
                uses: Some("actions/checkout@v4".to_string()),
                run: None,
                with: None,
                env: None,
                condition: None,
                working_directory: None,
                shell: None,
                continue_on_error: None,
                timeout_minutes: None,
            });
        }

        // Convert cigen steps to GitHub Actions steps
        if let Some(job_steps) = &job.steps {
            for cigen_step in job_steps {
                let step = self.build_step(cigen_step)?;
                steps.push(step);
            }
        }

        Ok(steps)
    }

    /// Build a single step
    fn build_step(&self, cigen_step: &crate::models::job::Step) -> Result<Step> {
        // Steps are stored as raw YAML values, we need to parse them
        let step_value = &cigen_step.0;

        // Try to extract common step fields from the YAML value
        if let Some(mapping) = step_value.as_mapping() {
            // Check if it's a run step
            if let Some(name_val) = mapping.get("name")
                && let Some(run_val) = mapping.get("run")
            {
                return Ok(Step {
                    id: None,
                    name: name_val.as_str().map(|s| s.to_string()),
                    uses: None,
                    run: run_val.as_str().map(|s| s.to_string()),
                    with: None,
                    env: None,
                    condition: None,
                    working_directory: None,
                    shell: None,
                    continue_on_error: None,
                    timeout_minutes: None,
                });
            }

            // Check if it's a uses step
            if let Some(uses_val) = mapping.get("uses") {
                return Ok(Step {
                    id: None,
                    name: mapping
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    uses: uses_val.as_str().map(|s| s.to_string()),
                    run: None,
                    with: None,
                    env: None,
                    condition: None,
                    working_directory: None,
                    shell: None,
                    continue_on_error: None,
                    timeout_minutes: None,
                });
            }

            // Check for string command reference (e.g. "command_name")
            if mapping.len() == 1
                && let Some((key, _)) = mapping.iter().next()
                && let Some(key_str) = key.as_str()
            {
                // This is a command reference
                miette::bail!("Command reference '{key_str}' not yet expanded for GitHub Actions")
            }
        }

        miette::bail!("Unsupported step format for GitHub Actions: {step_value:?}")
    }
}

impl Default for GitHubActionsGenerator {
    fn default() -> Self {
        Self::new()
    }
}
