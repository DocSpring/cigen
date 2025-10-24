use anyhow::{Context, Result};
use serde::Deserialize;
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::schema::{CigenConfig, CommandDefinition, Job, WorkflowConfig};

/// Root config metadata fields used by the loader
#[derive(Debug, Default, Deserialize)]
struct RootMetadata {
    provider: Option<String>,
    providers: Option<Vec<String>>,
    #[serde(default)]
    source_file_groups: HashMap<String, Vec<String>>,
}

/// Load split config from .cigen/ directory
pub fn load_split_config(config_dir: &Path) -> Result<CigenConfig> {
    // Read main config
    let config_path = config_dir.join("config.yml");
    let config_yaml = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;

    let mut merged_config: Value = serde_yaml::from_str(&config_yaml)
        .with_context(|| format!("Failed to parse {}", config_path.display()))?;

    // Merge optional fragments from .cigen/config/
    merge_config_fragments(config_dir, &mut merged_config)?;

    // Extract metadata for provider list + source file groups
    let raw_mapping = mapping_from_value(&merged_config);
    let metadata: RootMetadata = serde_yaml::from_value(Value::Mapping(raw_mapping.clone()))
        .context("Failed to deserialize merged configuration metadata")?;

    let providers = derive_providers(&metadata);

    let mut config = CigenConfig {
        project: None,
        providers,
        packages: vec![],
        source_file_groups: metadata.source_file_groups,
        jobs: HashMap::new(),
        commands: HashMap::new(),
        caches: HashMap::new(),
        runners: HashMap::new(),
        provider_config: HashMap::new(),
        workflows: HashMap::new(),
        raw: raw_mapping,
    };

    collect_provider_specific_blocks(&merged_config, &mut config);
    load_commands(config_dir, &mut config)?;
    load_jobs_and_workflows(config_dir, &mut config)?;

    Ok(config)
}

fn derive_providers(metadata: &RootMetadata) -> Vec<String> {
    if let Some(providers) = &metadata.providers {
        return providers.clone();
    }

    if let Some(provider) = &metadata.provider {
        return vec![match provider.as_str() {
            "github-actions" => "github".to_string(),
            "circleci" => "circleci".to_string(),
            other => other.to_string(),
        }];
    }

    Vec::new()
}

fn merge_config_fragments(config_dir: &Path, merged_config: &mut Value) -> Result<()> {
    let fragments_dir = config_dir.join("config");
    if !fragments_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&fragments_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("yml" | "yaml")
        ) {
            let fragment_yaml = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            let fragment_value: Value = serde_yaml::from_str(&fragment_yaml)
                .with_context(|| format!("Failed to parse {}", path.display()))?;
            merge_value(merged_config, fragment_value);
        }
    }

    Ok(())
}

fn load_commands(config_dir: &Path, config: &mut CigenConfig) -> Result<()> {
    let commands_dir = config_dir.join("commands");
    if !commands_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&commands_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file()
            || !matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("yml" | "yaml")
            )
        {
            continue;
        }

        let command_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid command filename {}", path.display()))?
            .to_string();

        let yaml = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let command: CommandDefinition = serde_yaml::from_str(&yaml)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        config.commands.insert(command_name, command);
    }

    Ok(())
}

fn load_jobs_and_workflows(config_dir: &Path, config: &mut CigenConfig) -> Result<()> {
    let workflows_dir = config_dir.join("workflows");
    if !workflows_dir.exists() {
        return Ok(());
    }

    for workflow_entry in fs::read_dir(&workflows_dir)? {
        let workflow_entry = workflow_entry?;
        let workflow_path = workflow_entry.path();

        if workflow_path.is_dir() {
            let workflow_name = workflow_path
                .file_name()
                .and_then(|s| s.to_str())
                .context("Invalid workflow directory name")?;

            let workflow_id = workflow_name.to_string();
            let workflow_config = load_workflow_config(&workflow_path)?;
            if !config.workflows.contains_key(&workflow_id) {
                config
                    .workflows
                    .insert(workflow_id.clone(), workflow_config);
            }

            let jobs_dir = workflow_path.join("jobs");
            if !jobs_dir.exists() {
                continue;
            }

            for job_file in fs::read_dir(&jobs_dir)? {
                let job_file = job_file?;
                let job_path = job_file.path();

                if !matches!(
                    job_path.extension().and_then(|s| s.to_str()),
                    Some("yml" | "yaml")
                ) {
                    continue;
                }

                let job_name = job_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .context("Invalid job filename")?
                    .to_string();

                let job_yaml = fs::read_to_string(&job_path)?;
                let mut job: Job = serde_yaml::from_str(&job_yaml)
                    .with_context(|| format!("Failed to parse {}", job_path.display()))?;

                job.workflow = Some(workflow_name.to_string());
                migrate_requires_to_needs(&mut job);

                config.jobs.insert(job_name, job);
            }
        } else if matches!(
            workflow_path.extension().and_then(|s| s.to_str()),
            Some("yml" | "yaml")
        ) {
            let workflow_name = workflow_path
                .file_stem()
                .and_then(|s| s.to_str())
                .context("Invalid workflow filename")?
                .to_string();

            let contents = fs::read_to_string(&workflow_path)?;
            let value: Value = serde_yaml::from_str(&contents)
                .with_context(|| format!("Failed to parse {}", workflow_path.display()))?;
            let workflow_config = WorkflowConfig::from_value(value)?;
            config.workflows.insert(workflow_name, workflow_config);
        }
    }

    Ok(())
}

fn load_workflow_config(workflow_path: &Path) -> Result<WorkflowConfig> {
    for candidate in ["config.yml", "config.yaml"] {
        let candidate_path = workflow_path.join(candidate);
        if candidate_path.exists() {
            let contents = fs::read_to_string(&candidate_path)
                .with_context(|| format!("Failed to read {}", candidate_path.display()))?;
            let value: Value = serde_yaml::from_str(&contents)
                .with_context(|| format!("Failed to parse {}", candidate_path.display()))?;
            return WorkflowConfig::from_value(value);
        }
    }

    Ok(WorkflowConfig::default())
}

fn migrate_requires_to_needs(job: &mut Job) {
    if !job.needs.is_empty() {
        return;
    }

    if let Some(requires_value) = job.extra.remove("requires")
        && let Some(seq) = requires_value.as_sequence()
    {
        let mut needs = Vec::new();
        for item in seq {
            if let Some(s) = item.as_str() {
                needs.push(s.to_string());
            }
        }
        if !needs.is_empty() {
            job.needs = needs;
        }
    }
}

fn collect_provider_specific_blocks(source: &Value, config: &mut CigenConfig) {
    let Value::Mapping(map) = source else {
        return;
    };

    for key in map.keys() {
        if let Some(provider_name) = key.as_str()
            && matches!(provider_name, "circleci" | "github" | "buildkite")
            && let Some(value) = map.get(key)
        {
            config
                .provider_config
                .insert(provider_name.to_string(), value.clone());
        }
    }
}

fn merge_value(dest: &mut Value, src: Value) {
    match (dest, src) {
        (Value::Mapping(dest_map), Value::Mapping(src_map)) => {
            for (key, value) in src_map {
                match dest_map.get_mut(&key) {
                    Some(existing) => merge_value(existing, value),
                    None => {
                        dest_map.insert(key, value);
                    }
                }
            }
        }
        (dest_value, src_value) => {
            *dest_value = src_value;
        }
    }
}

fn mapping_from_value(value: &Value) -> Mapping {
    match value {
        Value::Mapping(map) => map.clone(),
        other => {
            let mut map = Mapping::new();
            map.insert(Value::String("root".into()), other.clone());
            map
        }
    }
}
