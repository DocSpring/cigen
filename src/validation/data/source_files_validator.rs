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
            && let Some(span) = span_finder.find_field_span(&["source_files"])
        {
            // Check each source file entry for unknown group references
            let mut unknown_groups = Vec::new();
            for source_file in source_files {
                if let Some(group_name) = source_file.strip_prefix('@')
                    && !self.available_source_groups.contains(group_name)
                {
                    unknown_groups.push(group_name);
                }
            }

            if !unknown_groups.is_empty() {
                let err = DataValidationError::new(
                    file_path,
                    content.to_string(),
                    span,
                    format!(
                        "Unknown source file group(s) '{}'. Available groups: {}",
                        unknown_groups.join("', '"),
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
        }

        if job.source_submodules.is_some()
            && job.source_files.is_none()
            && let Some(span) = span_finder.find_field_span(&["source_submodules"])
        {
            let err = DataValidationError::new(
                file_path,
                content.to_string(),
                span,
                "source_submodules requires source_files to be configured".to_string(),
            );
            eprintln!();
            eprintln!("{:?}", miette::Report::new(err));
            return Err(anyhow::anyhow!("Data validation failed"));
        }

        Ok(())
    }
}
