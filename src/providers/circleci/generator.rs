use crate::models::config::ServiceEnvironment;
use crate::models::{Config, Job};
use crate::providers::circleci::config::{
    CircleCICommand, CircleCIConfig, CircleCIDockerAuth, CircleCIDockerImage, CircleCIJob,
    CircleCIStep, CircleCIWorkflow, CircleCIWorkflowJob, CircleCIWorkflowJobDetails,
};
use crate::providers::circleci::docker_images;
use crate::providers::circleci::template_commands;
use crate::validation::steps::StepValidator;
use miette::{IntoDiagnostic, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct CircleCIGenerator;

impl Default for CircleCIGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CircleCIGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_workflow(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
        commands: &HashMap<String, crate::models::Command>,
        output_path: &Path,
    ) -> Result<()> {
        let circleci_config = self.build_config(config, workflow_name, jobs, commands)?;

        // Write the YAML output
        let yaml_content = serde_yaml::to_string(&circleci_config).into_diagnostic()?;

        let output_file = if let Some(filename) = &config.output_filename {
            output_path.join(filename)
        } else {
            output_path.join("config.yml")
        };

        fs::create_dir_all(output_path).into_diagnostic()?;
        fs::write(&output_file, yaml_content).into_diagnostic()?;

        // Validate the generated config with CircleCI CLI
        self.validate_config(&output_file)?;

        Ok(())
    }

    pub fn generate_all(
        &self,
        config: &Config,
        workflows: &HashMap<String, HashMap<String, Job>>,
        commands: &HashMap<String, crate::models::Command>,
        output_path: &Path,
    ) -> Result<()> {
        // First validate all jobs across all workflows
        let validator = StepValidator::new();
        for workflow_jobs in workflows.values() {
            validator.validate_step_references(workflow_jobs, commands, "circleci")?;
        }

        let mut circleci_config = CircleCIConfig::default();

        // Process all workflows
        for (workflow_name, jobs) in workflows {
            let workflow_config = self.build_workflow(workflow_name, jobs)?;
            circleci_config
                .workflows
                .insert(workflow_name.clone(), workflow_config);

            // Process all jobs in the workflow
            for (job_name, job_def) in jobs {
                let circleci_job = self.convert_job(config, job_def)?;
                circleci_config.jobs.insert(job_name.clone(), circleci_job);
            }
        }

        // Add services as executors if any
        if let Some(services) = &config.services {
            circleci_config.executors = Some(self.build_executors(services)?);
        }

        // Scan for template command usage and add them to commands section
        let used_template_commands = self.scan_for_template_commands(&circleci_config);
        let used_user_commands = self.scan_for_user_commands(&circleci_config, commands);

        let mut all_commands = HashMap::new();

        // Add used template commands
        for cmd_name in used_template_commands {
            if let Some(cmd_def) = template_commands::get_template_command(&cmd_name) {
                let command: CircleCICommand = serde_yaml::from_value(cmd_def.clone())
                    .map_err(|e| miette::miette!("Failed to parse template command: {}", e))?;
                all_commands.insert(cmd_name, command);
            }
        }

        // Add used user commands
        for cmd_name in used_user_commands {
            if let Some(user_cmd) = commands.get(&cmd_name) {
                let command = self.convert_user_command(user_cmd)?;
                all_commands.insert(cmd_name, command);
            }
        }

        if !all_commands.is_empty() {
            circleci_config.commands = Some(all_commands);
        }

        // Write the YAML output
        let yaml_content = serde_yaml::to_string(&circleci_config).into_diagnostic()?;

        let output_file = if let Some(filename) = &config.output_filename {
            output_path.join(filename)
        } else {
            output_path.join("config.yml")
        };

        fs::create_dir_all(output_path).into_diagnostic()?;
        fs::write(&output_file, yaml_content).into_diagnostic()?;

        // Validate the generated config with CircleCI CLI
        self.validate_config(&output_file)?;

        Ok(())
    }

    pub fn build_config(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
        commands: &HashMap<String, crate::models::Command>,
    ) -> Result<CircleCIConfig> {
        // First validate that all step references are valid
        let validator = StepValidator::new();
        validator.validate_step_references(jobs, commands, "circleci")?;

        let mut circleci_config = CircleCIConfig::default();

        // Build workflow
        let workflow = self.build_workflow(workflow_name, jobs)?;
        circleci_config
            .workflows
            .insert(workflow_name.to_string(), workflow);

        // Convert jobs
        for (job_name, job_def) in jobs {
            let circleci_job = self.convert_job(config, job_def)?;
            circleci_config.jobs.insert(job_name.clone(), circleci_job);
        }

        // Add services as executors if any
        if let Some(services) = &config.services {
            circleci_config.executors = Some(self.build_executors(services)?);
        }

        // Handle setup workflows (either explicitly set or dynamic workflows)
        if config.setup.unwrap_or(false) || config.dynamic.unwrap_or(false) {
            circleci_config.setup = Some(true);
        }

        // Scan for template command usage and add them to commands section
        let used_template_commands = self.scan_for_template_commands(&circleci_config);
        let used_user_commands = self.scan_for_user_commands(&circleci_config, commands);

        let mut all_commands = HashMap::new();

        // Add used template commands
        for cmd_name in used_template_commands {
            if let Some(cmd_def) = template_commands::get_template_command(&cmd_name) {
                let command: CircleCICommand = serde_yaml::from_value(cmd_def.clone())
                    .map_err(|e| miette::miette!("Failed to parse template command: {}", e))?;
                all_commands.insert(cmd_name, command);
            }
        }

        // Add used user commands
        for cmd_name in used_user_commands {
            if let Some(user_cmd) = commands.get(&cmd_name) {
                let command = self.convert_user_command(user_cmd)?;
                all_commands.insert(cmd_name, command);
            }
        }

        if !all_commands.is_empty() {
            circleci_config.commands = Some(all_commands);
        }

        Ok(circleci_config)
    }

    fn build_workflow(
        &self,
        _workflow_name: &str,
        jobs: &HashMap<String, Job>,
    ) -> Result<CircleCIWorkflow> {
        let mut workflow_jobs = Vec::new();

        for (job_name, job_def) in jobs {
            // Handle job dependencies
            if let Some(requires) = &job_def.requires {
                let details = CircleCIWorkflowJobDetails {
                    requires: Some(requires.to_vec()),
                    context: None,
                    filters: None,
                    matrix: None,
                    name: None,
                    job_type: None,
                    pre_steps: None,
                    post_steps: None,
                };

                let mut job_map = HashMap::new();
                job_map.insert(job_name.clone(), details);

                workflow_jobs.push(CircleCIWorkflowJob::Detailed { job: job_map });
            } else {
                workflow_jobs.push(CircleCIWorkflowJob::Simple(job_name.clone()));
            }
        }

        Ok(CircleCIWorkflow {
            when: None,
            unless: None,
            jobs: workflow_jobs,
        })
    }

    pub(crate) fn convert_job(&self, config: &Config, job: &Job) -> Result<CircleCIJob> {
        let mut circleci_job = CircleCIJob {
            executor: None,
            docker: None,
            machine: None,
            resource_class: job.resource_class.clone(),
            working_directory: None,
            parallelism: job.parallelism,
            environment: None,
            parameters: None,
            steps: Vec::new(),
        };

        // Build Docker images
        let mut docker_images = vec![self.build_docker_image(config, &job.image)?];

        // Add service containers
        if let Some(service_refs) = &job.services
            && let Some(services) = &config.services
        {
            for service_ref in service_refs {
                if let Some(service) = services.get(service_ref) {
                    docker_images.push(self.build_service_image(config, service)?);
                }
            }
        }

        circleci_job.docker = Some(docker_images);

        // Add checkout step as the first step (standard CircleCI practice)
        // TODO: Make this configurable via job settings
        let checkout_step = serde_yaml::Value::String("checkout".to_string());
        circleci_job.steps.push(CircleCIStep::new(checkout_step));

        // Add restore_cache steps if specified
        if let Some(restore_caches) = &job.restore_cache {
            for cache in restore_caches {
                let cache_step = match cache {
                    crate::models::job::CacheRestore::Simple(name) => {
                        // Simple cache restoration - just use the cache name as key
                        let mut restore_step = serde_yaml::Mapping::new();
                        let mut restore_config = serde_yaml::Mapping::new();

                        // Generate cache key based on cache name
                        let cache_key = format!("{}-{{{{ checksum \"cache_key\" }}}}", name);
                        restore_config.insert(
                            serde_yaml::Value::String("keys".to_string()),
                            serde_yaml::Value::Sequence(vec![
                                serde_yaml::Value::String(cache_key.clone()),
                                serde_yaml::Value::String(format!("{}-", name)),
                            ]),
                        );

                        restore_step.insert(
                            serde_yaml::Value::String("restore_cache".to_string()),
                            serde_yaml::Value::Mapping(restore_config),
                        );

                        serde_yaml::Value::Mapping(restore_step)
                    }
                    crate::models::job::CacheRestore::Complex {
                        name,
                        dependency: _,
                    } => {
                        // Complex cache restoration with dependency flag
                        let mut restore_step = serde_yaml::Mapping::new();
                        let mut restore_config = serde_yaml::Mapping::new();

                        // If dependency is false, we might handle it differently
                        // For now, just restore the cache normally
                        let cache_key = format!("{}-{{{{ checksum \"cache_key\" }}}}", name);
                        restore_config.insert(
                            serde_yaml::Value::String("keys".to_string()),
                            serde_yaml::Value::Sequence(vec![
                                serde_yaml::Value::String(cache_key.clone()),
                                serde_yaml::Value::String(format!("{}-", name)),
                            ]),
                        );

                        restore_step.insert(
                            serde_yaml::Value::String("restore_cache".to_string()),
                            serde_yaml::Value::Mapping(restore_config),
                        );

                        serde_yaml::Value::Mapping(restore_step)
                    }
                };

                circleci_job.steps.push(CircleCIStep::new(cache_step));
            }
        }

        // Convert steps - just pass through the raw YAML
        if let Some(steps) = &job.steps {
            for step in steps {
                circleci_job.steps.push(CircleCIStep::new(step.0.clone()));
            }
        }

        Ok(circleci_job)
    }

    fn build_docker_image(&self, config: &Config, image: &str) -> Result<CircleCIDockerImage> {
        // Resolve docker image reference to actual image string
        // For now, don't pass architecture - that will be handled in architecture matrix support
        let resolved_image = docker_images::resolve_docker_image(image, None, config)
            .map_err(|e| miette::miette!("Failed to resolve docker image: {}", e))?;

        let mut docker_image = CircleCIDockerImage {
            image: resolved_image,
            auth: None,
            name: None,
            entrypoint: None,
            command: None,
            user: None,
            environment: None,
        };

        // Add authentication if configured
        if let Some(docker_config) = &config.docker
            && let Some(default_auth) = &docker_config.default_auth
            && let Some(auth_configs) = &docker_config.auth
            && let Some(auth) = auth_configs.get(default_auth)
        {
            docker_image.auth = Some(CircleCIDockerAuth {
                username: auth.username.clone(),
                password: auth.password.clone(),
            });
        }

        Ok(docker_image)
    }

    fn build_service_image(
        &self,
        config: &Config,
        service: &crate::models::Service,
    ) -> Result<CircleCIDockerImage> {
        let mut docker_image = CircleCIDockerImage {
            image: service.image.clone(),
            auth: None,
            name: None,
            entrypoint: None,
            command: None,
            user: None,
            environment: None,
        };

        // Add environment variables
        if let Some(env) = &service.environment {
            docker_image.environment = Some(match env {
                ServiceEnvironment::Map(map) => map.clone(),
                ServiceEnvironment::Array(arr) => {
                    // Convert array format ["KEY=value"] to HashMap
                    let mut env_map = HashMap::new();
                    for entry in arr {
                        if let Some((key, value)) = entry.split_once('=') {
                            env_map.insert(key.to_string(), value.to_string());
                        }
                    }
                    env_map
                }
            });
        }

        // Add authentication if specified
        if let Some(auth_name) = &service.auth
            && let Some(docker_config) = &config.docker
            && let Some(auth_configs) = &docker_config.auth
            && let Some(auth) = auth_configs.get(auth_name)
        {
            docker_image.auth = Some(CircleCIDockerAuth {
                username: auth.username.clone(),
                password: auth.password.clone(),
            });
        }

        Ok(docker_image)
    }

    fn build_executors(
        &self,
        _services: &HashMap<String, crate::models::Service>,
    ) -> Result<HashMap<String, crate::providers::circleci::config::CircleCIExecutor>> {
        // Placeholder for executor building logic
        // This would create reusable executors from service definitions
        Ok(HashMap::new())
    }

    fn validate_config(&self, config_file: &Path) -> Result<()> {
        // Check if CircleCI CLI is installed
        let cli_check = Command::new("circleci").arg("version").output();

        match cli_check {
            Ok(output) if output.status.success() => {
                // CLI is installed, run validation
                println!("Validating config with CircleCI CLI...");

                let validation_result = Command::new("circleci")
                    .arg("config")
                    .arg("validate")
                    .arg("-c")
                    .arg(config_file)
                    .output()
                    .into_diagnostic()?;

                if validation_result.status.success() {
                    println!("✓ Config file is valid");
                } else {
                    let stderr = String::from_utf8_lossy(&validation_result.stderr);
                    eprintln!("✗ Config validation failed:\n{stderr}");
                    return Err(miette::miette!("CircleCI config validation failed"));
                }
            }
            Ok(_) => {
                // CLI exists but version command failed
                println!("Warning: CircleCI CLI found but version check failed");
                self.print_install_instructions();
            }
            Err(_) => {
                // CLI not installed
                println!("CircleCI CLI not found - skipping validation");
                self.print_install_instructions();
            }
        }

        Ok(())
    }

    fn print_install_instructions(&self) {
        println!("\nTo enable config validation, install the CircleCI CLI:");
        println!("  brew install circleci");
        println!("  # or");
        println!(
            "  curl -fLSs https://raw.githubusercontent.com/CircleCI-Public/circleci-cli/main/install.sh | bash"
        );
        println!("  # or visit: https://circleci.com/docs/local-cli/");
    }

    fn scan_for_template_commands(&self, config: &CircleCIConfig) -> HashSet<String> {
        let mut used_commands = HashSet::new();

        // Scan all jobs for template command usage
        for job in config.jobs.values() {
            for step in &job.steps {
                if let Some(step_type) = &step.step_type
                    && template_commands::is_template_command(step_type)
                {
                    used_commands.insert(step_type.clone());
                }
            }
        }

        used_commands
    }

    fn scan_for_user_commands(
        &self,
        config: &CircleCIConfig,
        available_commands: &HashMap<String, crate::models::Command>,
    ) -> HashSet<String> {
        let mut used_commands = HashSet::new();

        // Scan all jobs for user command usage
        for job in config.jobs.values() {
            for step in &job.steps {
                // Check if the raw value is a string that matches a command name
                if let serde_yaml::Value::String(cmd_name) = &step.raw
                    && available_commands.contains_key(cmd_name)
                {
                    used_commands.insert(cmd_name.clone());
                }

                // Also check step_type for mapped commands
                if let Some(step_type) = &step.step_type
                    && available_commands.contains_key(step_type)
                {
                    used_commands.insert(step_type.clone());
                }
            }
        }

        used_commands
    }

    fn convert_user_command(&self, user_cmd: &crate::models::Command) -> Result<CircleCICommand> {
        // Convert cigen Command to CircleCICommand
        let mut steps = Vec::new();

        for step in &user_cmd.steps {
            // Convert old format to new CircleCI format
            if let Some(run) = &step.run {
                let mut step_map = serde_yaml::Mapping::new();
                let mut run_details = serde_yaml::Mapping::new();

                if let Some(name) = &step.name {
                    run_details.insert(
                        serde_yaml::Value::String("name".to_string()),
                        serde_yaml::Value::String(name.clone()),
                    );
                }

                run_details.insert(
                    serde_yaml::Value::String("command".to_string()),
                    serde_yaml::Value::String(run.clone()),
                );

                step_map.insert(
                    serde_yaml::Value::String("run".to_string()),
                    serde_yaml::Value::Mapping(run_details),
                );

                steps.push(CircleCIStep::new(serde_yaml::Value::Mapping(step_map)));
            }
        }

        Ok(CircleCICommand {
            description: Some(user_cmd.description.clone()),
            parameters: user_cmd.parameters.as_ref().map(|params| {
                params
                    .iter()
                    .map(|(k, v)| {
                        // For now, assume all parameters are strings
                        // TODO: Handle other parameter types based on param_type
                        let param = crate::providers::circleci::config::CircleCIParameter::String {
                            param_type: v.param_type.clone(),
                            description: v.description.clone(),
                            default: v
                                .default
                                .as_ref()
                                .and_then(|d| d.as_str().map(|s| s.to_string())),
                        };
                        (k.clone(), param)
                    })
                    .collect()
            }),
            steps,
        })
    }
}
