//! File scanning utilities for discovering YAML configuration files

use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct FileScanner;

impl FileScanner {
    /// Check if a file has a valid YAML extension
    pub fn is_yaml_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext, "yml" | "yaml" | "j2"))
            .unwrap_or(false)
    }

    /// Scan a directory for YAML files (non-recursive)
    pub fn scan_directory(dir: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        if !dir.exists() {
            return Ok(files);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && Self::is_yaml_file(&path) {
                files.push(path);
            }
        }

        Ok(files)
    }

    /// Scan for job files in the workflows directory structure
    pub fn scan_job_files(workflows_dir: &Path) -> Result<Vec<(PathBuf, String)>> {
        let mut job_files = Vec::new();

        if !workflows_dir.exists() || !workflows_dir.is_dir() {
            return Ok(job_files);
        }

        // Each subdirectory in workflows should contain a 'jobs' directory
        for entry in std::fs::read_dir(workflows_dir)? {
            let entry = entry?;
            let workflow_path = entry.path();

            if workflow_path.is_dir() {
                let workflow_name = workflow_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let jobs_dir = workflow_path.join("jobs");
                if jobs_dir.exists() && jobs_dir.is_dir() {
                    for job_file in Self::scan_directory(&jobs_dir)? {
                        job_files.push((job_file, workflow_name.clone()));
                    }
                }
            }
        }

        Ok(job_files)
    }

    /// Recursively scan for job files (used by validator)
    pub fn scan_job_files_recursive(dir: &Path) -> Result<Vec<PathBuf>> {
        let mut job_files = Vec::new();
        Self::scan_job_files_recursive_impl(dir, &mut job_files)?;
        Ok(job_files)
    }

    fn scan_job_files_recursive_impl(dir: &Path, job_files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                Self::scan_job_files_recursive_impl(&path, job_files)?;
            } else if path.is_file() {
                // Check if it's a YAML file in a jobs/ directory
                if let Some(parent) = path.parent()
                    && let Some(parent_name) = parent.file_name()
                    && parent_name == "jobs"
                    && Self::is_yaml_file(&path)
                {
                    job_files.push(path);
                }
            }
        }
        Ok(())
    }
}
