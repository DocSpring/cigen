use super::{DynamicPackageDetector, installer::PackageInstaller};
use crate::models::{
    Job,
    job::{CacheRestore, JobRequires},
};
use std::collections::HashMap;

/// Handles smart deduplication of package installation across jobs
pub struct PackageDeduplicator {
    detector: DynamicPackageDetector,
}

impl PackageDeduplicator {
    pub fn new(project_root: &str) -> Self {
        let config = DynamicPackageDetector::default_config();
        Self {
            detector: DynamicPackageDetector::new(project_root, config),
        }
    }

    /// Process all jobs and create dedicated installation jobs where beneficial
    /// Returns a new set of jobs with installation jobs added and dependencies configured
    pub fn process_jobs(&self, jobs: &mut HashMap<String, Job>) -> miette::Result<()> {
        // Group jobs by their package requirements
        let package_groups = self.group_jobs_by_packages(jobs)?;

        // For each package type, decide whether to create a dedicated job
        for (package_types, job_names) in package_groups {
            if job_names.len() > 1 {
                // Multiple jobs use the same packages - create dedicated installation job
                self.create_installation_job(jobs, &package_types, &job_names)?;
            } else if job_names.len() == 1 {
                // Single job - inline the installation steps
                self.inline_installation_steps(jobs, &job_names[0], &package_types)?;
            }
        }

        Ok(())
    }

    /// Group jobs by their package requirements
    fn group_jobs_by_packages(
        &self,
        jobs: &HashMap<String, Job>,
    ) -> miette::Result<HashMap<Vec<String>, Vec<String>>> {
        let mut groups: HashMap<Vec<String>, Vec<String>> = HashMap::new();

        for (job_name, job) in jobs {
            if let Some(packages) = &job.packages {
                let mut sorted_packages = packages.clone();
                sorted_packages.sort(); // Sort for consistent grouping

                groups
                    .entry(sorted_packages)
                    .or_default()
                    .push(job_name.clone());
            }
        }

        Ok(groups)
    }

    /// Create a dedicated installation job for shared packages
    fn create_installation_job(
        &self,
        jobs: &mut HashMap<String, Job>,
        package_types: &[String],
        job_names: &[String],
    ) -> miette::Result<()> {
        // Generate installation job name
        let install_job_name = format!("install_{}_packages", package_types.join("_"));

        // Get the appropriate image from one of the jobs
        let image = job_names
            .iter()
            .find_map(|name| jobs.get(name).map(|job| job.image.clone()))
            .unwrap_or_else(|| "cimg/base:stable".to_string());

        // Create the installation job
        let mut install_job = Job {
            image,
            packages: None,          // Don't set packages field on the generated job
            steps: Some(Vec::new()), // Will be filled with installation steps
            architectures: None,
            resource_class: None,
            source_files: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
        };

        // Add installation steps (checkout will be added automatically by CircleCI provider)
        let mut steps = Vec::new();

        for package_type in package_types {
            let detected = self.detector.detect_package_manager(package_type)?;
            // Just add the install command, not checkout
            steps.push(PackageInstaller::create_install_step(&detected.command));
        }

        install_job.steps = Some(steps);

        // Add the installation job
        jobs.insert(install_job_name.clone(), install_job);

        // Update dependent jobs
        for job_name in job_names {
            if let Some(job) = jobs.get_mut(job_name) {
                // Remove packages field (handled by installation job)
                job.packages = None;

                // Add dependency on installation job
                if let Some(requires) = &mut job.requires {
                    // Convert to vec, add new dependency, then convert back
                    let mut deps = requires.to_vec();
                    deps.push(install_job_name.clone());
                    job.requires = Some(if deps.len() == 1 {
                        JobRequires::Single(deps[0].clone())
                    } else {
                        JobRequires::Multiple(deps)
                    });
                } else {
                    job.requires = Some(JobRequires::Single(install_job_name.clone()));
                }

                // Add restore_cache for read-only access
                let cache_names: Vec<CacheRestore> = package_types
                    .iter()
                    .filter_map(|pt| self.get_cache_name_for_package(pt).ok())
                    .map(CacheRestore::Simple)
                    .collect();

                if !cache_names.is_empty() {
                    job.restore_cache = Some(cache_names);
                }
            }
        }

        Ok(())
    }

    /// Inline installation steps for a single job
    fn inline_installation_steps(
        &self,
        jobs: &mut HashMap<String, Job>,
        job_name: &str,
        package_types: &[String],
    ) -> miette::Result<()> {
        if let Some(job) = jobs.get_mut(job_name) {
            let mut install_steps = Vec::new();

            // Generate installation steps for each package type
            for package_type in package_types {
                let detected = self.detector.detect_package_manager(package_type)?;
                install_steps.extend(PackageInstaller::generate_install_steps(&detected));
            }

            // Prepend installation steps to existing steps
            if let Some(existing_steps) = &mut job.steps {
                install_steps.append(existing_steps);
                *existing_steps = install_steps;
            } else {
                job.steps = Some(install_steps);
            }

            // Clear packages field as it's been processed
            job.packages = None;
        }

        Ok(())
    }

    /// Get the cache name for a package type
    fn get_cache_name_for_package(&self, package_type: &str) -> miette::Result<String> {
        let detected = self.detector.detect_package_manager(package_type)?;
        Ok(detected.cache_config.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn create_test_jobs_with_multiple_packages() -> HashMap<String, Job> {
        let mut jobs = HashMap::new();

        jobs.insert(
            "test".to_string(),
            Job {
                image: "cimg/node:18".to_string(),
                packages: Some(vec!["node".to_string()]),
                steps: None,
                architectures: None,
                resource_class: None,
                source_files: None,
                parallelism: None,
                requires: None,
                cache: None,
                restore_cache: None,
                services: None,
            },
        );

        jobs.insert(
            "lint".to_string(),
            Job {
                image: "cimg/node:18".to_string(),
                packages: Some(vec!["node".to_string()]),
                steps: None,
                architectures: None,
                resource_class: None,
                source_files: None,
                parallelism: None,
                requires: None,
                cache: None,
                restore_cache: None,
                services: None,
            },
        );

        jobs.insert(
            "build".to_string(),
            Job {
                image: "cimg/node:18".to_string(),
                packages: Some(vec!["node".to_string()]),
                steps: None,
                architectures: None,
                resource_class: None,
                source_files: None,
                parallelism: None,
                requires: None,
                cache: None,
                restore_cache: None,
                services: None,
            },
        );

        jobs
    }

    #[test]
    fn test_deduplication_creates_install_job() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        // Create package-lock.json to enable npm detection
        fs::write(path.join("package-lock.json"), "{}").unwrap();

        let mut jobs = create_test_jobs_with_multiple_packages();
        let deduplicator = PackageDeduplicator::new(path.to_str().unwrap());

        deduplicator.process_jobs(&mut jobs).unwrap();

        // Should have created an installation job
        assert!(jobs.contains_key("install_node_packages"));

        // Installation job should NOT have packages field (it's been processed)
        let install_job = jobs.get("install_node_packages").unwrap();
        assert!(install_job.packages.is_none());

        // Original jobs should no longer have packages field
        assert!(jobs.get("test").unwrap().packages.is_none());
        assert!(jobs.get("lint").unwrap().packages.is_none());
        assert!(jobs.get("build").unwrap().packages.is_none());

        // Original jobs should depend on installation job
        let test_requires = jobs
            .get("test")
            .unwrap()
            .requires
            .as_ref()
            .unwrap()
            .to_vec();
        assert!(test_requires.contains(&"install_node_packages".to_string()));

        let lint_requires = jobs
            .get("lint")
            .unwrap()
            .requires
            .as_ref()
            .unwrap()
            .to_vec();
        assert!(lint_requires.contains(&"install_node_packages".to_string()));

        let build_requires = jobs
            .get("build")
            .unwrap()
            .requires
            .as_ref()
            .unwrap()
            .to_vec();
        assert!(build_requires.contains(&"install_node_packages".to_string()));

        // Original jobs should have restore_cache
        assert!(jobs.get("test").unwrap().restore_cache.is_some());
    }

    #[test]
    fn test_single_job_inline_installation() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        // Create package-lock.json
        fs::write(path.join("package-lock.json"), "{}").unwrap();

        let mut jobs = HashMap::from([(
            "test".to_string(),
            Job {
                image: "cimg/node:18".to_string(),
                packages: Some(vec!["node".to_string()]),
                steps: None,
                architectures: None,
                resource_class: None,
                source_files: None,
                parallelism: None,
                requires: None,
                cache: None,
                restore_cache: None,
                services: None,
            },
        )]);

        let deduplicator = PackageDeduplicator::new(path.to_str().unwrap());
        deduplicator.process_jobs(&mut jobs).unwrap();

        // Should NOT create an installation job
        assert!(!jobs.contains_key("install_node_packages"));

        // Job should have installation steps inlined
        let test_job = jobs.get("test").unwrap();
        assert!(test_job.steps.is_some());
        assert!(!test_job.steps.as_ref().unwrap().is_empty()); // install (checkout added by cigen later)

        // Packages field should be cleared
        assert!(test_job.packages.is_none());
    }

    #[test]
    fn test_installation_job_has_single_install_step() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        // Create package-lock.json to enable npm detection
        fs::write(path.join("package-lock.json"), "{}").unwrap();

        let mut jobs = create_test_jobs_with_multiple_packages();
        let deduplicator = PackageDeduplicator::new(path.to_str().unwrap());

        deduplicator.process_jobs(&mut jobs).unwrap();

        // Should have created an installation job
        assert!(jobs.contains_key("install_node_packages"));

        let install_job = jobs.get("install_node_packages").unwrap();
        let steps = install_job.steps.as_ref().unwrap();

        // Should have exactly 1 step (the install command)
        // Checkout will be added automatically by CircleCI provider
        assert_eq!(steps.len(), 1);

        // The single step should be the install command, not checkout
        let step_value = &steps[0].0;
        if let serde_yaml::Value::Mapping(step_map) = step_value {
            // Should have a "run" key for the install command
            assert!(step_map.contains_key("run"));
            // Should NOT be a simple "checkout" string
            assert!(!step_map.contains_key("checkout"));
        } else if let serde_yaml::Value::String(step_str) = step_value {
            // If it's a string, it should NOT be "checkout"
            assert_ne!(step_str, "checkout");
        }
    }
}
