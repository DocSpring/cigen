use anyhow::Result;
use std::path::Path;
use yaml_spanned::{Spanned, Value as YamlValue, from_str};

use super::error::DataValidationError;
use super::span_finder::SpanFinder;
use crate::models::{Config, Job};

pub struct DataValidator {
    file_content: String,
    file_path: String,
}

impl DataValidator {
    pub fn new(file_path: &Path, content: String) -> Self {
        Self {
            file_content: content,
            file_path: file_path.to_string_lossy().to_string(),
        }
    }

    /// Parse config with span tracking and perform data-level validations
    pub fn validate_config_data(&self) -> Result<Config> {
        // Parse YAML with spans
        let spanned_yaml: Spanned<YamlValue> = from_str(&self.file_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML with spans: {}", e))?;

        // Parse into our Config struct
        let config: Config = serde_yaml::from_str(&self.file_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;

        // Perform data-level validations
        self.validate_config_references(&config, &spanned_yaml)?;

        Ok(config)
    }

    /// Parse job with span tracking and perform data-level validations
    #[allow(dead_code)]
    pub fn validate_job_data(&self, config: &Config) -> Result<Job> {
        // Parse YAML with spans
        let spanned_yaml: Spanned<YamlValue> = from_str(&self.file_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML with spans: {}", e))?;

        // Parse into our Job struct
        let job: Job = serde_yaml::from_str(&self.file_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse job: {}", e))?;

        // Perform data-level validations
        self.validate_job_references(&job, config, &spanned_yaml)?;

        Ok(job)
    }

    fn validate_config_references(
        &self,
        config: &Config,
        spanned: &Spanned<YamlValue>,
    ) -> Result<()> {
        let span_finder = SpanFinder::new(spanned);

        // Validate docker auth references
        if let Some(docker) = &config.docker {
            if let (Some(default_auth), Some(auth_map)) = (&docker.default_auth, &docker.auth) {
                if !auth_map.contains_key(default_auth) {
                    // Find span for default_auth field
                    if let Some(span) = span_finder.find_field_span(&["docker", "default_auth"]) {
                        return Err(DataValidationError::new(
                            &self.file_path,
                            self.file_content.clone(),
                            span,
                            format!(
                                "Unknown docker auth '{}'. Available auth configurations: {}",
                                default_auth,
                                auth_map
                                    .keys()
                                    .map(|k| format!("'{k}'"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        )
                        .into());
                    }
                }
            }

            // Validate service auth references
            if let (Some(services), Some(auth_map)) = (&config.services, &docker.auth) {
                for (service_name, service) in services {
                    if let Some(auth_ref) = &service.auth {
                        if !auth_map.contains_key(auth_ref) {
                            if let Some(span) =
                                span_finder.find_field_span(&["services", service_name, "auth"])
                            {
                                return Err(DataValidationError::new(
                                    &self.file_path,
                                    self.file_content.clone(),
                                    span,
                                    format!(
                                        "Unknown docker auth '{}' for service '{}'. Available auth configurations: {}",
                                        auth_ref,
                                        service_name,
                                        auth_map.keys().map(|k| format!("'{k}'")).collect::<Vec<_>>().join(", ")
                                    ),
                                ).into());
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn validate_job_references(
        &self,
        job: &Job,
        config: &Config,
        spanned: &Spanned<YamlValue>,
    ) -> Result<()> {
        let span_finder = SpanFinder::new(spanned);
        let available_services = config.service_names();
        let available_cache_backends = config.cache_backend_names();
        let available_source_groups = config.source_file_group_names();

        // Validate service references
        for service_ref in job.service_references() {
            if !available_services.contains(&service_ref) {
                if let Some(span) = span_finder.find_array_item_span(&["services"], service_ref) {
                    return Err(DataValidationError::new(
                        &self.file_path,
                        self.file_content.clone(),
                        span,
                        format!(
                            "Unknown service '{}'. Available services: {}",
                            service_ref,
                            if available_services.is_empty() {
                                "none defined".to_string()
                            } else {
                                available_services
                                    .iter()
                                    .map(|s| format!("'{s}'"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        ),
                    )
                    .into());
                }
            }
        }

        // Validate cache references
        for cache_ref in job.cache_references() {
            if !available_cache_backends.contains(&cache_ref) {
                if let Some(span) = span_finder.find_cache_reference_span(cache_ref) {
                    return Err(DataValidationError::new(
                        &self.file_path,
                        self.file_content.clone(),
                        span,
                        format!(
                            "Unknown cache '{}'. Available cache backends: {}",
                            cache_ref,
                            if available_cache_backends.is_empty() {
                                "none defined".to_string()
                            } else {
                                available_cache_backends
                                    .iter()
                                    .map(|s| format!("'{s}'"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        ),
                    )
                    .into());
                }
            }
        }

        // Validate source file group reference
        if let Some(source_files) = &job.source_files {
            if !available_source_groups.contains(&source_files) {
                if let Some(span) = span_finder.find_field_span(&["source_files"]) {
                    return Err(DataValidationError::new(
                        &self.file_path,
                        self.file_content.clone(),
                        span,
                        format!(
                            "Unknown source file group '{}'. Available groups: {}",
                            source_files,
                            if available_source_groups.is_empty() {
                                "none defined".to_string()
                            } else {
                                available_source_groups
                                    .iter()
                                    .map(|s| format!("'{s}'"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        ),
                    )
                    .into());
                }
            }
        }

        Ok(())
    }
}
