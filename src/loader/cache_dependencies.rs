//! Infers job dependencies based on cache usage

use anyhow::Result;
use std::collections::{HashMap, HashSet};

use crate::models::Job;
use crate::models::job::{CacheRestore, JobRequires};

/// Infers and updates job dependencies based on cache usage patterns
pub fn infer_cache_dependencies(jobs: &mut HashMap<String, Job>) -> Result<()> {
    // First, build a map of which jobs produce which caches
    let mut cache_producers: HashMap<String, Vec<String>> = HashMap::new();

    for (job_name, job) in jobs.iter() {
        if let Some(cache_defs) = &job.cache {
            for cache_name in cache_defs.keys() {
                cache_producers
                    .entry(cache_name.clone())
                    .or_default()
                    .push(job_name.clone());
            }
        }
    }

    // Now, for each job that restores caches, add dependencies
    for (job_name, job) in jobs.iter_mut() {
        let mut inferred_deps = HashSet::new();

        // Get existing explicit dependencies
        if let Some(requires) = &job.requires {
            for dep in requires.to_vec() {
                // Handle relative job names
                let full_dep = if dep.contains('/') {
                    dep
                } else {
                    let workflow = job_name.split('/').next().unwrap_or("");
                    format!("{workflow}/{dep}")
                };
                inferred_deps.insert(full_dep);
            }
        }

        // Add dependencies based on cache restoration
        if let Some(restore_caches) = &job.restore_cache {
            for cache_ref in restore_caches {
                let (cache_name, is_dependency) = match cache_ref {
                    CacheRestore::Simple(name) => (name, true),
                    CacheRestore::Complex { name, dependency } => {
                        (name, dependency.unwrap_or(true))
                    }
                };

                // Skip if dependency: false
                if !is_dependency {
                    continue;
                }

                // Find jobs that produce this cache
                if let Some(producers) = cache_producers.get(cache_name) {
                    for producer in producers {
                        // Don't add self-dependency
                        if producer != job_name {
                            // Only add dependencies within the same workflow
                            let producer_workflow = producer.split('/').next().unwrap_or("");
                            let job_workflow = job_name.split('/').next().unwrap_or("");

                            if producer_workflow == job_workflow {
                                inferred_deps.insert(producer.clone());
                            }
                        }
                    }
                }
            }
        }

        // Update the job's requires field with the combined dependencies
        if !inferred_deps.is_empty() {
            let mut deps_vec: Vec<String> = inferred_deps.into_iter().collect();
            deps_vec.sort();

            // Convert back to relative job names for cleaner YAML
            let relative_deps: Vec<String> = deps_vec
                .into_iter()
                .map(|dep| {
                    let parts: Vec<&str> = dep.split('/').collect();
                    if parts.len() == 2 && parts[0] == job_name.split('/').next().unwrap_or("") {
                        parts[1].to_string()
                    } else {
                        dep
                    }
                })
                .collect();

            job.requires = Some(if relative_deps.len() == 1 {
                JobRequires::Single(relative_deps[0].clone())
            } else {
                JobRequires::Multiple(relative_deps)
            });
        }
    }

    Ok(())
}
