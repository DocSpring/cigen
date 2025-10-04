use crate::models::{Command, Job};
use crate::providers::circleci::schema::is_builtin_step;
use crate::providers::circleci::template_commands;
use miette::Result;
use std::collections::HashMap;

/// Validates that all step references in jobs are valid
pub struct StepValidator;

impl StepValidator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StepValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl StepValidator {
    /// Validate all step references in the given jobs
    pub fn validate_step_references(
        &self,
        jobs: &HashMap<String, Job>,
        available_commands: &HashMap<String, Command>,
        provider: &str,
    ) -> Result<()> {
        let mut errors = Vec::new();

        for (job_name, job) in jobs {
            if let Some(steps) = &job.steps {
                for (idx, step) in steps.iter().enumerate() {
                    if let Err(e) =
                        self.validate_step(&step.0, available_commands, provider, job_name, idx + 1)
                    {
                        errors.push(e);
                    }
                }
            }
        }

        if !errors.is_empty() {
            return Err(miette::miette!(
                "Step validation errors:\n{}",
                errors.join("\n")
            ));
        }

        Ok(())
    }

    fn validate_step(
        &self,
        step: &serde_yaml::Value,
        available_commands: &HashMap<String, Command>,
        provider: &str,
        job_name: &str,
        step_num: usize,
    ) -> Result<(), String> {
        match provider {
            "circleci" => self.validate_circleci_step(step, available_commands, job_name, step_num),
            _ => Ok(()), // Other providers not yet implemented
        }
    }

    fn validate_circleci_step(
        &self,
        step: &serde_yaml::Value,
        available_commands: &HashMap<String, Command>,
        job_name: &str,
        step_num: usize,
    ) -> Result<(), String> {
        match step {
            // String steps can be either builtin steps or command references
            serde_yaml::Value::String(cmd_name) => {
                // Check if it's a built-in step first
                if is_builtin_step(cmd_name) {
                    return Ok(());
                }

                // Check if it's a template or user command
                if !template_commands::is_template_command(cmd_name)
                    && !available_commands.contains_key(cmd_name)
                {
                    return Err(format!(
                        "Job '{job_name}' step {step_num}: command '{cmd_name}' is not defined"
                    ));
                }
                Ok(())
            }
            // Object steps should be valid step types
            serde_yaml::Value::Mapping(map) => {
                if map.len() == 1
                    && let Some((key, _)) = map.iter().next()
                    && let Some(step_type) = key.as_str()
                {
                    // Check if it's a built-in step type
                    if is_builtin_step(step_type) {
                        return Ok(());
                    }

                    // Check if it's a command reference
                    if template_commands::is_template_command(step_type)
                        || available_commands.contains_key(step_type)
                    {
                        return Ok(());
                    }

                    // Check if it's an orb command (contains /)
                    if step_type.contains('/') {
                        return Ok(());
                    }

                    // List of known CircleCI step types that don't need commands
                    let known_steps = [
                        "run",
                        "checkout",
                        "save_cache",
                        "restore_cache",
                        "store_artifacts",
                        "store_test_results",
                        "persist_to_workspace",
                        "attach_workspace",
                        "add_ssh_keys",
                        "setup_remote_docker",
                        "when",
                        "unless",
                        "deploy",
                        "commit_and_push_changed_files",
                    ];

                    if !known_steps.contains(&step_type) {
                        return Err(format!(
                            "Job '{job_name}' step {step_num}: unknown step type '{step_type}'"
                        ));
                    }
                }
                Ok(())
            }
            _ => Err(format!(
                "Job '{job_name}' step {step_num}: invalid step format"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::job::Step;

    #[test]
    fn test_valid_builtin_step() {
        let validator = StepValidator::new();
        let mut jobs = HashMap::new();

        let job = Job {
            image: "cimg/base".to_string(),
            steps: Some(vec![Step(serde_yaml::from_str("checkout").unwrap())]),
            architectures: None,
            resource_class: None,
            source_files: None,
            source_submodules: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            checkout: None,
            job_type: None,
            strategy: None,
            permissions: None,
            environment: None,
            concurrency: None,
            env: None,
        };

        jobs.insert("test".to_string(), job);
        let commands = HashMap::new();

        let result = validator.validate_step_references(&jobs, &commands, "circleci");
        if let Err(e) = &result {
            eprintln!("Validation error: {e}");
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_missing_command() {
        let validator = StepValidator::new();
        let mut jobs = HashMap::new();

        let job = Job {
            image: "cimg/base".to_string(),
            steps: Some(vec![Step(serde_yaml::Value::String(
                "nonexistent_command".to_string(),
            ))]),
            architectures: None,
            resource_class: None,
            source_files: None,
            source_submodules: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            checkout: None,
            job_type: None,
            strategy: None,
            permissions: None,
            environment: None,
            concurrency: None,
            env: None,
        };

        jobs.insert("test".to_string(), job);
        let commands = HashMap::new();

        let result = validator.validate_step_references(&jobs, &commands, "circleci");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("nonexistent_command"));
        assert!(err.contains("not defined"));
    }

    #[test]
    fn test_valid_user_command() {
        let validator = StepValidator::new();
        let mut jobs = HashMap::new();
        let mut commands = HashMap::new();

        // Add a user command
        let cmd = Command {
            description: "Test command".to_string(),
            parameters: None,
            steps: vec![],
        };
        commands.insert("my_command".to_string(), cmd);

        let job = Job {
            image: "cimg/base".to_string(),
            steps: Some(vec![Step(serde_yaml::Value::String(
                "my_command".to_string(),
            ))]),
            architectures: None,
            resource_class: None,
            source_files: None,
            source_submodules: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            checkout: None,
            job_type: None,
            strategy: None,
            permissions: None,
            environment: None,
            concurrency: None,
            env: None,
        };

        jobs.insert("test".to_string(), job);

        let result = validator.validate_step_references(&jobs, &commands, "circleci");
        assert!(result.is_ok());
    }
}
