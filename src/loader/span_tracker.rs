//! Tracks source file information for loaded configuration elements

use std::collections::HashMap;
use std::path::PathBuf;

/// Tracks the source files for configuration elements
#[derive(Debug, Default)]
pub struct SpanTracker {
    /// Maps job paths (e.g., "test/rspec") to their source file and content
    pub job_sources: HashMap<String, SourceInfo>,
}

#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub file_path: PathBuf,
    pub content: String,
}

impl SpanTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_job_source(&mut self, job_key: &str, file_path: PathBuf, content: String) {
        self.job_sources
            .insert(job_key.to_string(), SourceInfo { file_path, content });
    }

    pub fn get_job_source(&self, job_key: &str) -> Option<&SourceInfo> {
        self.job_sources.get(job_key)
    }
}
