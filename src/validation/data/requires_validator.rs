use anyhow::Result;
use std::collections::HashSet;
use yaml_spanned::{Spanned, Value as YamlValue};

use super::error::DataValidationError;
use super::span_finder::SpanFinder;
use crate::models::Job;
use crate::models::job::JobRequires;

pub struct RequiresValidator<'a> {
    all_job_names: &'a HashSet<&'a str>,
}

impl<'a> RequiresValidator<'a> {
    pub fn new(all_job_names: &'a HashSet<&'a str>) -> Self {
        Self { all_job_names }
    }

    pub fn validate_job_requires(
        &self,
        job: &Job,
        job_key: &str,
        file_path: &str,
        content: &str,
        spanned_yaml: &Spanned<YamlValue>,
    ) -> Result<()> {
        let span_finder = SpanFinder::new(spanned_yaml);

        if let Some(requires) = &job.requires {
            let required_jobs = requires.to_vec();
            for required_job in required_jobs {
                // Check if it's a simple job name (need to prepend current workflow)
                // or a full workflow/job path
                let full_job_key = if required_job.contains('/') {
                    required_job.clone()
                } else {
                    // Same workflow, just job name - extract workflow from current job_key
                    let workflow = job_key.split('/').next().unwrap_or("");
                    format!("{workflow}/{required_job}")
                };

                if !self.all_job_names.contains(full_job_key.as_str()) {
                    // Try to find the span for this requirement
                    let span = match &job.requires {
                        Some(JobRequires::Single(_)) => span_finder.find_field_span(&["requires"]),
                        Some(JobRequires::Multiple(_)) => {
                            span_finder.find_array_item_span(&["requires"], &required_job)
                        }
                        None => None,
                    };

                    if let Some(span) = span {
                        let err = DataValidationError::new(
                            file_path,
                            content.to_string(),
                            span,
                            format!(
                                "Unknown job '{}'. Available jobs: {}",
                                required_job,
                                if self.all_job_names.is_empty() {
                                    "none defined".to_string()
                                } else {
                                    let mut job_list: Vec<_> =
                                        self.all_job_names.iter().copied().collect();
                                    job_list.sort();
                                    // Show only jobs from the same workflow for brevity
                                    let current_workflow = job_key.split('/').next().unwrap_or("");
                                    let same_workflow_jobs: Vec<_> = job_list
                                        .iter()
                                        .filter(|j| j.starts_with(&format!("{current_workflow}/")))
                                        .map(|j| j.split('/').nth(1).unwrap_or(*j))
                                        .map(|j| format!("'{j}'"))
                                        .collect();

                                    if same_workflow_jobs.is_empty() {
                                        job_list
                                            .iter()
                                            .map(|s| format!("'{s}'"))
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    } else {
                                        format!(
                                            "in this workflow: {}",
                                            same_workflow_jobs.join(", ")
                                        )
                                    }
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
