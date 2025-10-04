use super::cache::CacheGenerator;
use super::job_skip::JobSkipGenerator;
use super::schema::{Job as GHJob, RunsOn, Step, Workflow};
use crate::models::{Command, Config, Job};
use miette::{IntoDiagnostic, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct GitHubActionsGenerator {
    cache_generator: CacheGenerator,
    job_skip_generator: JobSkipGenerator,
}

impl GitHubActionsGenerator {
    pub fn new() -> Self {
        Self {
            cache_generator: CacheGenerator::new(),
            job_skip_generator: JobSkipGenerator::new(),
        }
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
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
    ) -> Result<Workflow> {
        let mut gh_jobs = HashMap::new();

        for (job_name, job) in jobs {
            let gh_job = self.build_job(config, job)?;
            gh_jobs.insert(job_name.clone(), gh_job);
        }

        // Get workflow-specific config if available
        let workflow_config = config
            .workflows
            .as_ref()
            .and_then(|workflows| workflows.get(workflow_name));

        // Build workflow triggers - use explicit 'on' or generate defaults
        let triggers = if let Some(wf_config) = workflow_config
            && let Some(on_value) = &wf_config.on
        {
            Some(serde_yaml::from_value(on_value.clone()).into_diagnostic()?)
        } else {
            self.build_workflow_triggers(config, workflow_name)
        };

        // Parse permissions if provided
        let permissions = if let Some(wf_config) = workflow_config
            && let Some(perms_value) = &wf_config.permissions
        {
            Some(serde_yaml::from_value(perms_value.clone()).into_diagnostic()?)
        } else {
            None
        };

        // Parse concurrency if provided
        let concurrency = if let Some(wf_config) = workflow_config
            && let Some(conc_value) = &wf_config.concurrency
        {
            Some(serde_yaml::from_value(conc_value.clone()).into_diagnostic()?)
        } else {
            None
        };

        // Get env vars
        let env = workflow_config.and_then(|wf| wf.env.clone());

        Ok(Workflow {
            name: workflow_name.to_string(),
            on: triggers,
            permissions,
            jobs: gh_jobs,
            env,
            concurrency,
        })
    }

    /// Build workflow triggers from config
    fn build_workflow_triggers(
        &self,
        config: &Config,
        workflow_name: &str,
    ) -> Option<super::schema::WorkflowTrigger> {
        // Check if there's workflow-specific configuration
        if let Some(workflows) = &config.workflows
            && let Some(_workflow_config) = workflows.get(workflow_name)
        {
            // For now, use sensible defaults for CI workflows
            // TODO: Parse trigger configuration from workflow config
            return Some(self.default_ci_triggers());
        }

        // Default triggers for CI workflows
        Some(self.default_ci_triggers())
    }

    /// Generate default CI triggers (push to main, pull requests)
    fn default_ci_triggers(&self) -> super::schema::WorkflowTrigger {
        use super::schema::{TriggerConfig, WorkflowTrigger};
        use std::collections::HashMap;

        let mut triggers = HashMap::new();

        // Trigger on push to main
        triggers.insert(
            "push".to_string(),
            TriggerConfig {
                branches: Some(vec!["main".to_string()]),
                tags: None,
                paths: None,
                types: None,
            },
        );

        // Trigger on pull requests to main
        triggers.insert(
            "pull_request".to_string(),
            TriggerConfig {
                branches: Some(vec!["main".to_string()]),
                tags: None,
                paths: None,
                types: None,
            },
        );

        WorkflowTrigger::Detailed(triggers)
    }

    /// Build a GitHub Actions job from a cigen job
    fn build_job(&self, config: &Config, job: &Job) -> Result<GHJob> {
        let steps = self.build_steps(config, job)?;

        // Convert requires to needs (GitHub Actions only supports AND dependencies)
        let needs = job.requires.as_ref().map(|r| r.to_vec());

        // Build matrix strategy - prefer explicit strategy field, fall back to architectures
        let strategy = if let Some(strategy_value) = &job.strategy {
            // Parse the YAML value into our Strategy type
            Some(serde_yaml::from_value(strategy_value.clone()).into_diagnostic()?)
        } else {
            self.build_matrix_strategy(job)?
        };

        // Build services
        let services = self.build_services(job)?;

        // Build conditional expression
        let condition = self.build_job_condition(job);

        // Parse permissions if provided
        let permissions = if let Some(perms_value) = &job.permissions {
            Some(serde_yaml::from_value(perms_value.clone()).into_diagnostic()?)
        } else {
            None
        };

        // Parse environment if provided
        let environment = if let Some(env_value) = &job.environment {
            Some(serde_yaml::from_value(env_value.clone()).into_diagnostic()?)
        } else {
            None
        };

        // Determine runs_on from image field or default
        let runs_on = Some(
            if job.image.starts_with("ubuntu") || job.image.starts_with("rust") {
                RunsOn::Single("ubuntu-latest".to_string())
            } else if job.image.starts_with("macos") {
                RunsOn::Single("macos-latest".to_string())
            } else {
                RunsOn::Single(job.image.clone())
            },
        );

        Ok(GHJob {
            name: None, // GitHub Actions infers job name from key
            runs_on,
            needs,
            condition,
            steps: Some(steps),
            env: job.env.clone(),
            strategy,
            services,
            container: None,
            timeout_minutes: None,
            outputs: None,
            permissions,
            environment,
        })
    }

    /// Build conditional expression for a job
    /// In the future, this will handle requires_any by generating OR conditions
    fn build_job_condition(&self, _job: &Job) -> Option<String> {
        // TODO: Check for requires_any field (when added to Job model)
        // If requires_any is present, generate:
        // "needs.job1.result == 'success' || needs.job2.result == 'success'"
        //
        // For now, return None (no conditions)
        None
    }

    /// Build matrix strategy from job architectures
    fn build_matrix_strategy(&self, job: &Job) -> Result<Option<super::schema::Strategy>> {
        if let Some(architectures) = &job.architectures
            && architectures.len() > 1
        {
            use super::schema::Strategy;
            use std::collections::HashMap;

            let mut matrix = HashMap::new();

            // Add architecture dimension
            let arch_values: Vec<serde_json::Value> = architectures
                .iter()
                .map(|a| serde_json::Value::String(a.clone()))
                .collect();

            matrix.insert("arch".to_string(), arch_values);

            return Ok(Some(Strategy {
                matrix,
                fail_fast: None,
                max_parallel: None,
            }));
        }

        Ok(None)
    }

    /// Build service containers from job services
    fn build_services(&self, job: &Job) -> Result<Option<HashMap<String, super::schema::Service>>> {
        if let Some(service_refs) = &job.services
            && !service_refs.is_empty()
        {
            // TODO: Look up service definitions from config and convert to GH Actions format
            // For now, return None - will implement in service containers task
            return Ok(None);
        }

        Ok(None)
    }

    /// Build steps from a cigen job
    fn build_steps(&self, config: &Config, job: &Job) -> Result<Vec<Step>> {
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

        // Determine architecture - use first from architectures or default to amd64
        let architecture = job
            .architectures
            .as_ref()
            .and_then(|archs| archs.first())
            .map(|s| s.as_str())
            .unwrap_or("amd64");

        // Add job skip logic if source_files are defined
        let skip_steps = self
            .job_skip_generator
            .generate_skip_steps(config, job, architecture)?;

        if let Some((hash_step, skip_check_step, completion_step)) = skip_steps {
            // Add hash calculation step
            steps.push(hash_step);

            // Add skip check step
            steps.push(skip_check_step);

            // Add automatic cache restoration
            let cache_steps =
                self.cache_generator
                    .generate_cache_steps(config, job, architecture)?;
            steps.extend(cache_steps);

            // Convert cigen steps to GitHub Actions steps
            if let Some(job_steps) = &job.steps {
                for cigen_step in job_steps {
                    let step = self.build_step(cigen_step)?;
                    steps.push(step);
                }
            }

            // Add completion recording step at the end
            steps.push(completion_step);
        } else {
            // No skip logic - add cache and user steps normally
            let cache_steps =
                self.cache_generator
                    .generate_cache_steps(config, job, architecture)?;
            steps.extend(cache_steps);

            // Convert cigen steps to GitHub Actions steps
            if let Some(job_steps) = &job.steps {
                for cigen_step in job_steps {
                    let step = self.build_step(cigen_step)?;
                    steps.push(step);
                }
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
            // Check if it's a CircleCI run step with command
            if let Some(run_mapping) = mapping.get("run")
                && let Some(run_map) = run_mapping.as_mapping()
            {
                let name = run_map
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let command = run_map
                    .get("command")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                if let Some(cmd) = command {
                    return Ok(Step {
                        id: None,
                        name,
                        uses: None,
                        run: Some(cmd),
                        with: None,
                        env: None,
                        condition: None,
                        working_directory: None,
                        shell: None,
                        continue_on_error: None,
                        timeout_minutes: None,
                    });
                }
            }

            // Check if it's a native GitHub Actions run step
            if let Some(name_val) = mapping.get("name")
                && let Some(run_val) = mapping.get("run")
            {
                return Ok(Step {
                    id: mapping
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
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
                // Parse with parameters if present
                let with_params = mapping.get("with").and_then(|v| {
                    if let Some(with_map) = v.as_mapping() {
                        let mut params = std::collections::HashMap::new();
                        for (k, val) in with_map {
                            if let Some(key_str) = k.as_str() {
                                // Convert YAML value to JSON value
                                if let Ok(json_val) = serde_json::to_value(val) {
                                    params.insert(key_str.to_string(), json_val);
                                }
                            }
                        }
                        Some(params)
                    } else {
                        None
                    }
                });

                // Parse env if present
                let env_vars = mapping.get("env").and_then(|v| {
                    if let Some(env_map) = v.as_mapping() {
                        let mut vars = std::collections::HashMap::new();
                        for (k, val) in env_map {
                            if let (Some(key_str), Some(val_str)) = (k.as_str(), val.as_str()) {
                                vars.insert(key_str.to_string(), val_str.to_string());
                            }
                        }
                        Some(vars)
                    } else {
                        None
                    }
                });

                return Ok(Step {
                    id: mapping
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    name: mapping
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    uses: uses_val.as_str().map(|s| s.to_string()),
                    run: None,
                    with: with_params,
                    env: env_vars,
                    condition: mapping
                        .get("if")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    working_directory: mapping
                        .get("working-directory")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    shell: mapping
                        .get("shell")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    continue_on_error: mapping.get("continue-on-error").and_then(|v| v.as_bool()),
                    timeout_minutes: mapping
                        .get("timeout-minutes")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
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
