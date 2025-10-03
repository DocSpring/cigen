use super::schema::Step;
use crate::models::config::CacheDefinition as ConfigCacheDefinition;
use crate::models::job::CacheRestore;
use crate::models::{Config, Job};
use miette::Result;
use std::collections::HashMap;

/// Generate cache steps for GitHub Actions using actions/cache@v4
pub struct CacheGenerator {}

impl CacheGenerator {
    pub fn new() -> Self {
        Self {}
    }

    /// Generate cache restore steps from job cache configuration
    /// Note: GitHub Actions cache automatically saves on job success (post-action)
    /// so we only need to generate restore steps
    pub fn generate_cache_steps(
        &self,
        config: &Config,
        job: &Job,
        architecture: &str,
    ) -> Result<Vec<Step>> {
        let mut cache_steps = Vec::new();

        // Add automatic cache restoration based on job.cache field
        // This implements convention-over-configuration: declaring a cache automatically injects restore steps
        if let Some(cache_defs) = &job.cache {
            for (cache_name, cache_def) in cache_defs {
                // Only restore caches that have restore enabled (default is true)
                if cache_def.restore {
                    let step = self.build_cache_step_from_job_cache(
                        config,
                        cache_name,
                        &cache_def.paths,
                        architecture,
                    )?;
                    cache_steps.push(step);
                }
            }
        }

        // Add restore_cache steps if explicitly specified (legacy support)
        if let Some(restore_caches) = &job.restore_cache {
            for cache in restore_caches {
                let cache_name = match cache {
                    CacheRestore::Simple(name) => name,
                    CacheRestore::Complex { name, .. } => name,
                };

                let step = self.build_cache_step_from_restore(config, cache_name, architecture)?;
                cache_steps.push(step);
            }
        }

        Ok(cache_steps)
    }

    /// Build a cache step from job.cache definition
    fn build_cache_step_from_job_cache(
        &self,
        config: &Config,
        cache_name: &str,
        paths: &[String],
        architecture: &str,
    ) -> Result<Step> {
        // Get cache configuration from config.cache_definitions
        let cache_config = config
            .cache_definitions
            .as_ref()
            .and_then(|defs| defs.get(cache_name));

        let (key, restore_keys) =
            self.generate_cache_keys(config, cache_name, cache_config, architecture)?;

        // Build the with parameters for actions/cache@v4
        let mut with = HashMap::new();
        with.insert("path".to_string(), serde_json::json!(paths.join("\n")));
        with.insert("key".to_string(), serde_json::json!(key));
        with.insert(
            "restore-keys".to_string(),
            serde_json::json!(restore_keys.join("\n")),
        );

        Ok(Step {
            id: None,
            name: Some(format!("Restore {cache_name} cache")),
            uses: Some("actions/cache@v4".to_string()),
            run: None,
            with: Some(with),
            env: None,
            condition: None,
            working_directory: None,
            shell: None,
            continue_on_error: None,
            timeout_minutes: None,
        })
    }

    /// Build a cache step from restore_cache reference
    fn build_cache_step_from_restore(
        &self,
        config: &Config,
        cache_name: &str,
        architecture: &str,
    ) -> Result<Step> {
        // Look up the cache definition in config.cache_definitions
        let cache_config = config
            .cache_definitions
            .as_ref()
            .and_then(|defs| defs.get(cache_name));

        let (key, restore_keys) =
            self.generate_cache_keys(config, cache_name, cache_config, architecture)?;

        // Get paths from cache definition
        let paths = if let Some(cache_config) = cache_config
            && let Some(paths_spec) = &cache_config.paths
        {
            // Extract paths from PathOrDetect enum
            paths_spec
                .iter()
                .filter_map(|p| match p {
                    crate::models::config::PathOrDetect::Path(path) => Some(path.clone()),
                    _ => None, // TODO: Handle detect patterns
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        let mut with = HashMap::new();
        with.insert("path".to_string(), serde_json::json!(paths.join("\n")));
        with.insert("key".to_string(), serde_json::json!(key));
        with.insert(
            "restore-keys".to_string(),
            serde_json::json!(restore_keys.join("\n")),
        );

        Ok(Step {
            id: None,
            name: Some(format!("Restore {cache_name} cache")),
            uses: Some("actions/cache@v4".to_string()),
            run: None,
            with: Some(with),
            env: None,
            condition: None,
            working_directory: None,
            shell: None,
            continue_on_error: None,
            timeout_minutes: None,
        })
    }

    /// Generate cache key and restore keys
    fn generate_cache_keys(
        &self,
        _config: &Config,
        cache_name: &str,
        cache_config: Option<&ConfigCacheDefinition>,
        architecture: &str,
    ) -> Result<(String, Vec<String>)> {
        // Build the cache key using the cache name from config's cache_definitions
        let key = if let Some(cache_config) = cache_config
            && let Some(key_template) = &cache_config.key
        {
            // Use the key template from cache_definitions
            // Replace {{ arch }} with the actual architecture
            // Convert CircleCI {{ checksum(...) }} to GitHub Actions ${{ hashFiles(...) }}
            self.convert_cache_key_template(key_template, architecture)
        } else {
            // No key template, use a reasonable default
            // GitHub Actions format: runner.os is the OS, runner.arch is the architecture
            format!(
                "${{{{ runner.os }}}}-{architecture}-{cache_name}-${{{{ hashFiles('**/lockfile') }}}}"
            )
        };

        // Generate restore keys (prefix patterns for fallback)
        let restore_keys = vec![
            // Same OS/arch/name, any checksum
            format!("${{{{ runner.os }}}}-{architecture}-{cache_name}-"),
        ];

        Ok((key, restore_keys))
    }

    /// Convert CircleCI cache key template to GitHub Actions format
    fn convert_cache_key_template(&self, template: &str, architecture: &str) -> String {
        // Replace {{ arch }} with the actual architecture
        let result = template.replace("{{ arch }}", architecture);

        // Convert CircleCI {{ checksum("file") }} or {{ checksum "file" }} to GitHub Actions ${{ hashFiles('file') }}
        // Need to handle both quoted and unquoted forms, with optional parentheses
        let re = regex::Regex::new(r#"\{\{\s*checksum\s*\(?\s*["']([^"']+)["']\s*\)?\s*\}\}"#)
            .expect("Invalid regex");
        let result = re.replace_all(&result, "$${{ hashFiles('$1') }}");

        result.to_string()
    }
}

impl Default for CacheGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_generator_creation() {
        let generator = CacheGenerator::new();
        let config = Config::default();
        let job = Job {
            image: "rust:latest".to_string(),
            architectures: None,
            resource_class: None,
            source_files: None,
            source_submodules: None,
            parallelism: None,
            requires: None,
            cache: None,
            restore_cache: None,
            services: None,
            packages: None,
            steps: None,
            checkout: None,
            job_type: None,
        };

        let cache_steps = generator
            .generate_cache_steps(&config, &job, "amd64")
            .unwrap();
        assert_eq!(cache_steps.len(), 0); // No cache defined
    }

    #[test]
    fn test_cache_key_template_conversion() {
        let generator = CacheGenerator::new();

        // Test {{ arch }} replacement
        let result = generator.convert_cache_key_template(
            "linux-{{ arch }}-gems-{{ checksum \"Gemfile.lock\" }}",
            "amd64",
        );
        assert_eq!(result, "linux-amd64-gems-${{ hashFiles('Gemfile.lock') }}");

        // Test with double quotes
        let result =
            generator.convert_cache_key_template("{{ checksum \"package-lock.json\" }}", "amd64");
        assert_eq!(result, "${{ hashFiles('package-lock.json') }}");

        // Test with single quotes
        let result = generator.convert_cache_key_template("{{ checksum 'Cargo.lock' }}", "arm64");
        assert_eq!(result, "${{ hashFiles('Cargo.lock') }}");
    }

    #[test]
    fn test_cache_step_generation_with_job_cache() {
        use crate::models::config::CacheDefinition as ConfigCacheDefinition;
        use crate::models::job::CacheDefinition as JobCacheDefinition;
        use std::collections::HashMap;

        let generator = CacheGenerator::new();

        // Create config with cache definition
        let mut cache_definitions = HashMap::new();
        cache_definitions.insert(
            "gems".to_string(),
            ConfigCacheDefinition {
                key: Some("linux-{{ arch }}-gems-{{ checksum \"Gemfile.lock\" }}".to_string()),
                versions: None,
                checksum_sources: None,
                paths: Some(vec![crate::models::config::PathOrDetect::Path(
                    "vendor/bundle".to_string(),
                )]),
                backend: None,
                config: None,
            },
        );

        let config = Config {
            provider: "github-actions".to_string(),
            cache_definitions: Some(cache_definitions),
            ..Default::default()
        };

        // Create job with cache
        let mut job_caches = HashMap::new();
        job_caches.insert(
            "gems".to_string(),
            JobCacheDefinition {
                paths: vec!["vendor/bundle".to_string()],
                restore: true,
            },
        );

        let job = Job {
            image: "ruby:3.3".to_string(),
            cache: Some(job_caches),
            architectures: Some(vec!["amd64".to_string()]),
            ..Job {
                image: String::new(),
                architectures: None,
                resource_class: None,
                source_files: None,
                source_submodules: None,
                parallelism: None,
                requires: None,
                cache: None,
                restore_cache: None,
                services: None,
                packages: None,
                steps: None,
                checkout: None,
                job_type: None,
            }
        };

        let cache_steps = generator
            .generate_cache_steps(&config, &job, "amd64")
            .unwrap();

        // Should have generated one cache restore step
        assert_eq!(cache_steps.len(), 1);

        let step = &cache_steps[0];
        assert_eq!(step.uses, Some("actions/cache@v4".to_string()));
        assert_eq!(step.name, Some("Restore gems cache".to_string()));

        // Check the with parameters
        let with = step.with.as_ref().unwrap();
        assert_eq!(with.get("path").unwrap(), "vendor/bundle");
        assert_eq!(
            with.get("key").unwrap(),
            "linux-amd64-gems-${{ hashFiles('Gemfile.lock') }}"
        );

        // Check restore-keys has the prefix fallback
        let restore_keys = with.get("restore-keys").unwrap().as_str().unwrap();
        assert!(restore_keys.contains("${{ runner.os }}-amd64-gems-"));
    }

    #[test]
    fn test_cache_step_generation_with_restore_cache() {
        use crate::models::config::CacheDefinition as ConfigCacheDefinition;
        use crate::models::job::CacheRestore;
        use std::collections::HashMap;

        let generator = CacheGenerator::new();

        // Create config with cache definition
        let mut cache_definitions = HashMap::new();
        cache_definitions.insert(
            "node_modules".to_string(),
            ConfigCacheDefinition {
                key: Some("${{ runner.os }}-node-{{ checksum \"package-lock.json\" }}".to_string()),
                versions: None,
                checksum_sources: None,
                paths: Some(vec![crate::models::config::PathOrDetect::Path(
                    "node_modules".to_string(),
                )]),
                backend: None,
                config: None,
            },
        );

        let config = Config {
            provider: "github-actions".to_string(),
            cache_definitions: Some(cache_definitions),
            ..Default::default()
        };

        // Create job with restore_cache
        let job = Job {
            image: "node:20".to_string(),
            restore_cache: Some(vec![CacheRestore::Simple("node_modules".to_string())]),
            ..Job {
                image: String::new(),
                architectures: None,
                resource_class: None,
                source_files: None,
                source_submodules: None,
                parallelism: None,
                requires: None,
                cache: None,
                restore_cache: None,
                services: None,
                packages: None,
                steps: None,
                checkout: None,
                job_type: None,
            }
        };

        let cache_steps = generator
            .generate_cache_steps(&config, &job, "amd64")
            .unwrap();

        // Should have generated one cache restore step
        assert_eq!(cache_steps.len(), 1);

        let step = &cache_steps[0];
        assert_eq!(step.uses, Some("actions/cache@v4".to_string()));
        assert_eq!(step.name, Some("Restore node_modules cache".to_string()));
    }
}
