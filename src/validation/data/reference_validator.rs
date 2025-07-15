use anyhow::Result;
use std::collections::{HashMap, HashSet};

use crate::loader::span_tracker::SpanTracker;
use crate::models::{Config, Job};

use super::cache_validator::CacheValidator;
use super::requires_validator::RequiresValidator;
use super::service_validator::ServiceValidator;
use super::source_files_validator::SourceFilesValidator;

pub struct ReferenceValidator;

impl ReferenceValidator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReferenceValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceValidator {
    /// Validate all references across the entire configuration
    pub fn validate_all_references(
        &self,
        config: &Config,
        jobs: &HashMap<String, Job>,
        span_tracker: &SpanTracker,
    ) -> Result<()> {
        // First, collect ALL available resources across the entire configuration

        // 1. Services from config
        let available_services: HashSet<&str> = config
            .services
            .as_ref()
            .map(|s| s.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();

        // 2. Source file groups from config
        let available_source_groups: HashSet<&str> = config
            .source_file_groups
            .as_ref()
            .map(|s| s.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();

        // 3. ALL cache definitions from ALL jobs
        let mut all_defined_caches: HashSet<String> = HashSet::new();
        for job in jobs.values() {
            if let Some(cache_defs) = &job.cache {
                for cache_name in cache_defs.keys() {
                    all_defined_caches.insert(cache_name.clone());
                }
            }
        }

        // 4. ALL job names (for requires validation)
        let all_job_names: HashSet<&str> = jobs.keys().map(|k| k.as_str()).collect();

        // Create validators
        let service_validator = ServiceValidator::new(&available_services);
        let cache_validator = CacheValidator::new(&all_defined_caches);
        let source_files_validator = SourceFilesValidator::new(&available_source_groups);
        let requires_validator = RequiresValidator::new(&all_job_names);

        // Now validate each job's references
        for (job_key, job) in jobs {
            if let Some(source_info) = span_tracker.get_job_source(job_key) {
                // Re-parse with spans for error reporting
                let spanned_yaml: yaml_spanned::Spanned<yaml_spanned::Value> =
                    yaml_spanned::from_str(&source_info.content)
                        .map_err(|e| anyhow::anyhow!("Failed to parse YAML with spans: {}", e))?;

                let file_path = source_info.file_path.to_string_lossy();

                // Validate services
                service_validator.validate_job_services(
                    job,
                    &file_path,
                    &source_info.content,
                    &spanned_yaml,
                )?;

                // Validate source files
                source_files_validator.validate_job_source_files(
                    job,
                    &file_path,
                    &source_info.content,
                    &spanned_yaml,
                )?;

                // Validate caches
                cache_validator.validate_job_caches(
                    job,
                    &file_path,
                    &source_info.content,
                    &spanned_yaml,
                )?;

                // Validate requires
                requires_validator.validate_job_requires(
                    job,
                    job_key,
                    &file_path,
                    &source_info.content,
                    &spanned_yaml,
                )?;
            }
        }

        Ok(())
    }
}
