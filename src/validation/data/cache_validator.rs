use anyhow::Result;
use std::collections::HashSet;
use yaml_spanned::{Spanned, Value as YamlValue};

use super::error::DataValidationError;
use super::span_finder::SpanFinder;
use crate::models::Job;
use crate::models::job::CacheRestore;

pub struct CacheValidator<'a> {
    all_defined_caches: &'a HashSet<String>,
}

impl<'a> CacheValidator<'a> {
    pub fn new(all_defined_caches: &'a HashSet<String>) -> Self {
        Self { all_defined_caches }
    }

    pub fn validate_job_caches(
        &self,
        job: &Job,
        file_path: &str,
        content: &str,
        spanned_yaml: &Spanned<YamlValue>,
    ) -> Result<()> {
        let span_finder = SpanFinder::new(spanned_yaml);

        if let Some(restore_caches) = &job.restore_cache {
            for cache_ref in restore_caches {
                let cache_name = match cache_ref {
                    CacheRestore::Simple(name) => name,
                    CacheRestore::Complex { name, .. } => name,
                };

                if !self.all_defined_caches.contains(cache_name) {
                    if let Some(span) = span_finder.find_cache_reference_span(cache_name) {
                        let err = DataValidationError::new(
                            file_path,
                            content.to_string(),
                            span,
                            format!(
                                "Unknown cache '{}'. Defined caches: {}",
                                cache_name,
                                if self.all_defined_caches.is_empty() {
                                    "none defined".to_string()
                                } else {
                                    let mut cache_list: Vec<_> = self
                                        .all_defined_caches
                                        .iter()
                                        .map(|s| s.as_str())
                                        .collect();
                                    cache_list.sort();
                                    cache_list
                                        .iter()
                                        .map(|s| format!("'{s}'"))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                }
                            ),
                        );
                        eprintln!();
                        eprintln!("{:?}", miette::Report::new(err));
                        return Err(anyhow::anyhow!("Data validation failed"));
                    }
                }
            }
        }

        Ok(())
    }
}
