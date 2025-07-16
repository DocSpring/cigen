use crate::models::config::ServiceEnvironment;
use crate::models::job::Step;
use crate::models::{Config, Job};
use crate::providers::circleci::config::{
    CircleCIConfig, CircleCIDockerAuth, CircleCIDockerImage, CircleCIJob, CircleCIRunDetails,
    CircleCIRunStep, CircleCIStep, CircleCIWorkflow, CircleCIWorkflowJob,
    CircleCIWorkflowJobDetails,
};
use miette::{IntoDiagnostic, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct CircleCIGenerator;

impl CircleCIGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_workflow(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
        output_path: &Path,
    ) -> Result<()> {
        let circleci_config = self.build_config(config, workflow_name, jobs)?;

        // Write the YAML output
        let yaml_content = serde_yaml::to_string(&circleci_config).into_diagnostic()?;

        let output_file = if let Some(filename) = &config.output_filename {
            output_path.join(filename)
        } else {
            output_path.join("config.yml")
        };

        fs::create_dir_all(output_path).into_diagnostic()?;
        fs::write(output_file, yaml_content).into_diagnostic()?;

        Ok(())
    }

    pub fn generate_all(
        &self,
        config: &Config,
        workflows: &HashMap<String, HashMap<String, Job>>,
        output_path: &Path,
    ) -> Result<()> {
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

        // Write the YAML output
        let yaml_content = serde_yaml::to_string(&circleci_config).into_diagnostic()?;

        let output_file = if let Some(filename) = &config.output_filename {
            output_path.join(filename)
        } else {
            output_path.join("config.yml")
        };

        fs::create_dir_all(output_path).into_diagnostic()?;
        fs::write(output_file, yaml_content).into_diagnostic()?;

        Ok(())
    }

    pub fn build_config(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
    ) -> Result<CircleCIConfig> {
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

        // Handle dynamic workflows
        if config.dynamic.unwrap_or(false) {
            circleci_config.setup = Some(true);
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

    fn convert_job(&self, config: &Config, job: &Job) -> Result<CircleCIJob> {
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
        if let Some(service_refs) = &job.services {
            if let Some(services) = &config.services {
                for service_ref in service_refs {
                    if let Some(service) = services.get(service_ref) {
                        docker_images.push(self.build_service_image(config, service)?);
                    }
                }
            }
        }

        circleci_job.docker = Some(docker_images);

        // Convert steps
        if let Some(steps) = &job.steps {
            // Add checkout step first
            circleci_job
                .steps
                .push(CircleCIStep::Checkout { checkout: None });

            // TODO: Cache restore steps will be handled by the caching module

            // Convert regular steps
            for step in steps {
                circleci_job.steps.push(self.convert_step(step)?);
            }

            // TODO: Cache save steps will be handled by the caching module
        }

        Ok(circleci_job)
    }

    fn build_docker_image(&self, config: &Config, image: &str) -> Result<CircleCIDockerImage> {
        let mut docker_image = CircleCIDockerImage {
            image: image.to_string(),
            auth: None,
            name: None,
            entrypoint: None,
            command: None,
            user: None,
            environment: None,
        };

        // Add authentication if configured
        if let Some(docker_config) = &config.docker {
            if let Some(default_auth) = &docker_config.default_auth {
                if let Some(auth_configs) = &docker_config.auth {
                    if let Some(auth) = auth_configs.get(default_auth) {
                        docker_image.auth = Some(CircleCIDockerAuth {
                            username: auth.username.clone(),
                            password: auth.password.clone(),
                        });
                    }
                }
            }
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
        if let Some(auth_name) = &service.auth {
            if let Some(docker_config) = &config.docker {
                if let Some(auth_configs) = &docker_config.auth {
                    if let Some(auth) = auth_configs.get(auth_name) {
                        docker_image.auth = Some(CircleCIDockerAuth {
                            username: auth.username.clone(),
                            password: auth.password.clone(),
                        });
                    }
                }
            }
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

    fn convert_step(&self, step: &Step) -> Result<CircleCIStep> {
        match step {
            Step::Command(cmd) => Ok(CircleCIStep::Run(CircleCIRunStep::Simple {
                run: cmd.clone(),
            })),
            Step::Complex {
                name,
                run,
                store_test_results,
                store_artifacts,
            } => {
                if let Some(run_cmd) = run {
                    Ok(CircleCIStep::Run(CircleCIRunStep::Detailed {
                        run: CircleCIRunDetails {
                            name: name.clone(),
                            command: run_cmd.to_string(),
                            shell: None,
                            environment: None,
                            background: None,
                            working_directory: None,
                            no_output_timeout: None,
                            when: None,
                        },
                    }))
                } else if let Some(test_results) = store_test_results {
                    Ok(CircleCIStep::StoreTestResults {
                        store_test_results:
                            crate::providers::circleci::config::CircleCIStoreTestResults {
                                path: test_results.path.clone(),
                            },
                    })
                } else if let Some(artifacts) = store_artifacts {
                    Ok(CircleCIStep::StoreArtifacts {
                        store_artifacts:
                            crate::providers::circleci::config::CircleCIStoreArtifacts {
                                path: artifacts.path.clone(),
                                destination: None,
                            },
                    })
                } else {
                    // Default to a named run step
                    Ok(CircleCIStep::Run(CircleCIRunStep::Detailed {
                        run: CircleCIRunDetails {
                            name: name.clone(),
                            command: "echo 'No command specified'".to_string(),
                            shell: None,
                            environment: None,
                            background: None,
                            working_directory: None,
                            no_output_timeout: None,
                            when: None,
                        },
                    }))
                }
            }
        }
    }
}
