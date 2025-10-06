/// Conversion from plugin protocol types to cigen model types
/// This allows us to use the existing GitHubActionsGenerator with plugin data
use cigen::models::{Config, Job};
use cigen::plugin::protocol::{CigenSchema, JobDefinition};
use std::collections::HashMap;

/// Convert a CigenSchema (from protocol) to a minimal Config (for generator)
pub fn schema_to_config(schema: &CigenSchema) -> Config {
    // Create a minimal config that the generator needs
    // Most fields can be None since the generator has sensible defaults
    Config {
        provider: "github".to_string(),
        output_path: None,
        output_filename: None,
        version: Some(1),
        anchors: None,
        caches: None,
        cache_definitions: None,
        version_sources: None,
        architectures: None,
        resource_classes: None,
        docker: None,
        services: None,
        source_file_groups: None,
        vars: None,
        graph: None,
        dynamic: None,
        setup: None,
        parameters: None,
        orbs: None,
        outputs: None,
        docker_images: None,
        docker_build: None,
        package_managers: None,
        workflows: None,
        workflows_meta: None,
        checkout: None,
        setup_options: None,
    }
}

/// Convert a JobDefinition (from protocol) to a Job (for generator)
pub fn job_def_to_job(job_def: &JobDefinition) -> Job {
    use cigen::models::job::Step as ModelStep;
    use cigen::plugin::protocol::step::StepType;

    // Convert steps
    let steps = job_def
        .steps
        .iter()
        .filter_map(|step| {
            if let Some(step_type) = &step.step_type {
                match step_type {
                    StepType::Run(run) => {
                        // Create a YAML value for a run step
                        let step_map = if run.name.is_empty() {
                            serde_yaml::to_value(serde_json::json!({
                                "run": run.command
                            }))
                        } else {
                            serde_yaml::to_value(serde_json::json!({
                                "name": run.name,
                                "run": run.command
                            }))
                        };

                        step_map.ok().map(ModelStep)
                    }
                    StepType::Uses(uses) => {
                        // Create a YAML value for a uses step
                        let mut step_obj = serde_json::json!({
                            "uses": uses.module
                        });

                        if !uses.name.is_empty() {
                            step_obj["name"] = serde_json::Value::String(uses.name.clone());
                        }

                        if !uses.with.is_empty() {
                            step_obj["with"] = serde_json::to_value(&uses.with).unwrap();
                        }

                        serde_yaml::to_value(step_obj).ok().map(ModelStep)
                    }
                }
            } else {
                None
            }
        })
        .collect();

    Job {
        image: "ubuntu-latest".to_string(), // Default to ubuntu
        requires: if job_def.needs.is_empty() {
            None
        } else {
            Some(job_def.needs.clone())
        },
        requires_any: None,
        steps: Some(steps),
        packages: job_def.packages.clone(),
        source_files: None,
        dependencies: None,
        env: if job_def.env.is_empty() {
            None
        } else {
            Some(job_def.env.clone())
        },
        architectures: None,
        resource_class: None,
        checkout: None,
        services: None,
        docker_build: None,
        artifacts: None,
        parallelism: None,
        timeout: None,
        runner: None,
        strategy: None,
        permissions: None,
        environment: None,
    }
}

/// Convert all jobs from schema to a HashMap of Jobs
pub fn convert_jobs(schema: &CigenSchema) -> HashMap<String, Job> {
    schema
        .jobs
        .iter()
        .map(|job_def| (job_def.id.clone(), job_def_to_job(job_def)))
        .collect()
}
