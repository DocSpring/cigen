use anyhow::Result;
use std::collections::HashSet;
use yaml_spanned::{Spanned, Value as YamlValue};

use super::error::DataValidationError;
use super::span_finder::SpanFinder;
use crate::models::Job;

pub struct ServiceValidator<'a> {
    available_services: &'a HashSet<&'a str>,
}

impl<'a> ServiceValidator<'a> {
    pub fn new(available_services: &'a HashSet<&'a str>) -> Self {
        Self { available_services }
    }

    pub fn validate_job_services(
        &self,
        job: &Job,
        file_path: &str,
        content: &str,
        spanned_yaml: &Spanned<YamlValue>,
    ) -> Result<()> {
        let span_finder = SpanFinder::new(spanned_yaml);

        if let Some(services) = &job.services {
            for service in services {
                if !self.available_services.contains(service.as_str()) {
                    if let Some(span) = span_finder.find_array_item_span(&["services"], service) {
                        let err = DataValidationError::new(
                            file_path,
                            content.to_string(),
                            span,
                            format!(
                                "Unknown service '{}'. Available services: {}",
                                service,
                                if self.available_services.is_empty() {
                                    "none defined".to_string()
                                } else {
                                    self.available_services
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
