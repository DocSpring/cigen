use anyhow::Result;
use std::collections::HashSet;
use yaml_spanned::{Spanned, Value as YamlValue};

use super::error::DataValidationError;
use super::span_finder::SpanFinder;
use crate::models::Job;

pub struct SourceFilesValidator<'a> {
    available_source_groups: &'a HashSet<&'a str>,
}

impl<'a> SourceFilesValidator<'a> {
    pub fn new(available_source_groups: &'a HashSet<&'a str>) -> Self {
        Self {
            available_source_groups,
        }
    }

    pub fn validate_job_source_files(
        &self,
        job: &Job,
        file_path: &str,
        content: &str,
        spanned_yaml: &Spanned<YamlValue>,
    ) -> Result<()> {
        let span_finder = SpanFinder::new(spanned_yaml);

        if let Some(source_files) = &job.source_files
            && !self.available_source_groups.contains(source_files.as_str())
            && let Some(span) = span_finder.find_field_span(&["source_files"])
        {
            let err = DataValidationError::new(
                file_path,
                content.to_string(),
                span,
                format!(
                    "Unknown source file group '{}'. Available groups: {}",
                    source_files,
                    if self.available_source_groups.is_empty() {
                        "none defined".to_string()
                    } else {
                        self.available_source_groups
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

        Ok(())
    }
}
