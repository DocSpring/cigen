use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::schema::{CigenConfig, Job};

/// Temporary struct for loading legacy .cigen/config.yml format
#[derive(serde::Deserialize)]
struct LegacyConfig {
    provider: Option<String>,
    providers: Option<Vec<String>>,
}

/// Load split config from .cigen/ directory
pub fn load_split_config(config_dir: &Path) -> Result<CigenConfig> {
    // Read main config
    let config_path = config_dir.join("config.yml");
    let config_yaml = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;

    // Parse to get provider(s)
    let legacy: LegacyConfig = serde_yaml::from_str(&config_yaml)?;

    let providers = if let Some(providers) = legacy.providers {
        providers
    } else if let Some(provider) = legacy.provider {
        // Convert legacy singular provider to list
        vec![match provider.as_str() {
            "github-actions" => "github".to_string(),
            "circleci" => "circleci".to_string(),
            other => other.to_string(),
        }]
    } else {
        vec![]
    };

    let mut config = CigenConfig {
        project: None,
        providers,
        packages: vec![],
        jobs: HashMap::new(),
        caches: HashMap::new(),
        runners: HashMap::new(),
        provider_config: HashMap::new(),
    };

    // Load jobs from workflows directory
    let workflows_dir = config_dir.join("workflows");
    if workflows_dir.exists() {
        for workflow_entry in fs::read_dir(&workflows_dir)? {
            let workflow_entry = workflow_entry?;
            let workflow_path = workflow_entry.path();

            if workflow_path.is_dir() {
                let workflow_name = workflow_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .context("Invalid workflow directory name")?;

                let jobs_dir = workflow_path.join("jobs");
                if jobs_dir.exists() {
                    for job_file in fs::read_dir(&jobs_dir)? {
                        let job_file = job_file?;
                        let job_path = job_file.path();

                        if job_path.extension().and_then(|s| s.to_str()) == Some("yml")
                            || job_path.extension().and_then(|s| s.to_str()) == Some("yaml")
                        {
                            let job_name = job_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .context("Invalid job filename")?
                                .to_string();

                            let job_yaml = fs::read_to_string(&job_path)?;
                            let mut job: Job =
                                serde_yaml::from_str(&job_yaml).with_context(|| {
                                    format!("Failed to parse {}", job_path.display())
                                })?;

                            // Set the workflow this job belongs to
                            job.workflow = Some(workflow_name.to_string());

                            config.jobs.insert(job_name, job);
                        }
                    }
                }
            }
        }
    }

    Ok(config)
}
