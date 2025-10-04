//! Job-specific loading logic

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

use super::file_scanner::FileScanner;
use super::span_tracker::SpanTracker;
use crate::models::{Config, Job};
use crate::templating::TemplateEngine;

pub struct JobLoader<'a> {
    template_engine: &'a mut TemplateEngine,
}

impl<'a> JobLoader<'a> {
    pub fn new(template_engine: &'a mut TemplateEngine) -> Self {
        Self { template_engine }
    }

    /// Load all jobs from the workflows directory or inline config
    pub fn load_all_jobs(
        &mut self,
        config: &Config,
        span_tracker: &mut SpanTracker,
    ) -> Result<HashMap<String, Job>> {
        let mut jobs = HashMap::new();

        // First check if workflows are defined inline in the main config
        if self.has_inline_workflows(config) {
            // Extract jobs from inline workflow definitions
            return self.load_jobs_from_inline_config(config, span_tracker);
        }

        // Otherwise, look for split workflow files in directories
        let current_dir = std::env::current_dir()?;
        let is_in_cigen_dir = current_dir.file_name() == Some(std::ffi::OsStr::new(".cigen"));

        let workflows_dir = if is_in_cigen_dir {
            Path::new("workflows")
        } else if current_dir.join(".cigen/workflows").exists() {
            Path::new(".cigen/workflows")
        } else {
            Path::new("workflows")
        };

        // Validate workflows directory exists
        if !workflows_dir.exists() {
            anyhow::bail!(
                "Missing required 'workflows' directory in {}",
                current_dir.display()
            );
        }

        if !workflows_dir.is_dir() {
            anyhow::bail!("'workflows' must be a directory, not a file");
        }

        // Scan and load all job files
        let scanned_files = FileScanner::scan_job_files(workflows_dir)?;
        for (job_path, workflow_name) in scanned_files {
            self.load_job_file(&job_path, &workflow_name, &mut jobs, span_tracker)?;
        }

        // Validate workflow structure
        self.validate_workflow_structure(workflows_dir)?;

        Ok(jobs)
    }

    /// Load a single job file
    fn load_job_file(
        &mut self,
        path: &Path,
        workflow_name: &str,
        jobs: &mut HashMap<String, Job>,
        span_tracker: &mut SpanTracker,
    ) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        let processed_content = self.process_file_content(path, &content)?;

        let job: Job = serde_yaml::from_str(&processed_content)
            .with_context(|| format!("Failed to parse job file {}", path.display()))?;

        let job_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid job filename: {}", path.display()))?;

        // Key format: "workflow_name/job_name"
        let key = format!("{workflow_name}/{job_name}");

        if jobs.contains_key(&key) {
            anyhow::bail!("Duplicate job key: {}", key);
        }

        // Track the source file for this job
        span_tracker.add_job_source(&key, path.to_path_buf(), content);

        jobs.insert(key, job);
        Ok(())
    }

    /// Validate that each workflow has a jobs directory
    fn validate_workflow_structure(&self, workflows_dir: &Path) -> Result<()> {
        for entry in std::fs::read_dir(workflows_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let workflow_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let jobs_dir = path.join("jobs");

                if !jobs_dir.exists() {
                    anyhow::bail!(
                        "Workflow '{}' is missing required 'jobs' directory at: {}",
                        workflow_name,
                        jobs_dir.display()
                    );
                }

                if !jobs_dir.is_dir() {
                    anyhow::bail!(
                        "In workflow '{}', 'jobs' must be a directory, not a file",
                        workflow_name
                    );
                }
            }
        }
        Ok(())
    }

    /// Process file content with templating if needed
    fn process_file_content(&mut self, path: &Path, content: &str) -> Result<String> {
        let is_template = crate::templating::TemplateEngine::is_template_file(path);
        self.template_engine
            .render_file_with_path(content, path, is_template)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    /// Check if the config has inline workflows defined
    fn has_inline_workflows(&self, config: &Config) -> bool {
        // Only consider it inline if workflows exist AND at least one has jobs defined
        if let Some(workflows) = &config.workflows {
            workflows.values().any(|wf| wf.jobs.is_some())
        } else {
            false
        }
    }

    /// Load jobs from inline workflow definitions in the main config
    fn load_jobs_from_inline_config(
        &mut self,
        config: &Config,
        _span_tracker: &mut SpanTracker,
    ) -> Result<HashMap<String, Job>> {
        let mut jobs = HashMap::new();

        if let Some(workflows) = &config.workflows {
            for (workflow_name, workflow_config) in workflows {
                if let Some(workflow_jobs) = &workflow_config.jobs {
                    for (job_name, job) in workflow_jobs {
                        // Create a path key like "workflow_name/job_name"
                        let job_path = format!("{}/{}", workflow_name, job_name);
                        jobs.insert(job_path, job.clone());
                    }
                }
            }
        }

        Ok(jobs)
    }
}
