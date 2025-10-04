use super::schema::Step;
use crate::models::{Config, Job};
use miette::Result;

/// Generate job skipping steps for GitHub Actions
/// Uses early-exit pattern since GitHub Actions doesn't support dynamic workflow generation
pub struct JobSkipGenerator {}

impl JobSkipGenerator {
    pub fn new() -> Self {
        Self {}
    }

    /// Generate job skip steps if job has source_files defined
    /// Returns (hash_step, skip_check_step, completion_step)
    pub fn generate_skip_steps(
        &self,
        config: &Config,
        job: &Job,
        architecture: &str,
    ) -> Result<Option<(Step, Step, Step)>> {
        if let Some(source_files) = &job.source_files {
            let hash_step = self.build_hash_calculation_step(config, source_files)?;
            let skip_check_step = self.build_skip_check_step(architecture)?;
            let completion_step = self.build_completion_step(architecture)?;

            Ok(Some((hash_step, skip_check_step, completion_step)))
        } else {
            Ok(None)
        }
    }

    /// Build hash calculation step for source files
    fn build_hash_calculation_step(
        &self,
        config: &Config,
        source_files: &[String],
    ) -> Result<Step> {
        let source_file_groups = config.source_file_groups.as_ref();

        // Expand groups into concrete patterns
        let mut patterns: Vec<String> = Vec::new();
        for entry in source_files {
            if let Some(group_name) = entry.strip_prefix('@') {
                if let Some(groups) = source_file_groups {
                    let file_patterns = groups.get(group_name).ok_or_else(|| {
                        miette::miette!(
                            "source_files group '{group_name}' not found in source_file_groups"
                        )
                    })?;
                    patterns.extend(file_patterns.clone());
                } else {
                    miette::bail!("source_file_groups not defined in config");
                }
            } else {
                patterns.push(entry.clone());
            }
        }

        // Build shell command to calculate hash
        let mut hash_lines = vec![
            "set -e".to_string(),
            "echo 'Calculating source file hash...'".to_string(),
            "TEMP_HASH_FILE=\"/tmp/source_files_for_hash\"".to_string(),
            "rm -f \"$TEMP_HASH_FILE\"".to_string(),
            "".to_string(),
        ];

        // Add each pattern to the hash file
        for pattern in &patterns {
            // Use find for glob patterns, escaping special characters
            let escaped_pattern = pattern.replace('\'', "'\\''");
            hash_lines.push(format!(
                "find . -path './{escaped_pattern}' -type f 2>/dev/null | sort >> \"$TEMP_HASH_FILE\" || true"
            ));
        }

        hash_lines.extend(vec![
            "".to_string(),
            "if [ -s \"$TEMP_HASH_FILE\" ]; then".to_string(),
            "  # Calculate hash of file contents".to_string(),
            "  JOB_HASH=$(xargs -I{{}} sh -c 'cat \"{{}}\"' < \"$TEMP_HASH_FILE\" | sha256sum | cut -d' ' -f1)".to_string(),
            "  echo \"Hash calculated: $JOB_HASH\"".to_string(),
            "  echo \"JOB_HASH=$JOB_HASH\" >> $GITHUB_ENV".to_string(),
            "else".to_string(),
            "  JOB_HASH=\"empty\"".to_string(),
            "  echo \"No source files found, using empty hash\"".to_string(),
            "  echo \"JOB_HASH=$JOB_HASH\" >> $GITHUB_ENV".to_string(),
            "fi".to_string(),
        ]);

        Ok(Step {
            id: Some("calculate_hash".to_string()),
            name: Some("Calculate source file hash".to_string()),
            uses: None,
            run: Some(hash_lines.join("\n")),
            with: None,
            env: None,
            condition: None,
            working_directory: None,
            shell: Some("bash".to_string()),
            continue_on_error: None,
            timeout_minutes: None,
        })
    }

    /// Build skip check step that exits early if job was already completed
    fn build_skip_check_step(&self, architecture: &str) -> Result<Step> {
        let command = format!(
            r#"set -e
SKIP_CACHE_DIR="/tmp/cigen_skip_cache"
SKIP_MARKER="$SKIP_CACHE_DIR/job_${{{{ env.JOB_HASH }}}}_{}"

if [ -f "$SKIP_MARKER" ]; then
  echo "✓ Job already completed successfully for this file hash. Skipping..."
  exit 0
else
  echo "→ No previous successful run found. Proceeding with job..."
  mkdir -p "$SKIP_CACHE_DIR"
fi"#,
            architecture
        );

        Ok(Step {
            id: Some("check_skip".to_string()),
            name: Some("Check if job should be skipped".to_string()),
            uses: None,
            run: Some(command),
            with: None,
            env: None,
            condition: None,
            working_directory: None,
            shell: Some("bash".to_string()),
            continue_on_error: None,
            timeout_minutes: None,
        })
    }

    /// Build completion recording step
    fn build_completion_step(&self, architecture: &str) -> Result<Step> {
        let command = format!(
            r#"set -e
SKIP_CACHE_DIR="/tmp/cigen_skip_cache"
SKIP_MARKER="$SKIP_CACHE_DIR/job_${{{{ env.JOB_HASH }}}}_{}"

echo "Recording successful completion for hash ${{{{ env.JOB_HASH }}}}"
mkdir -p "$SKIP_CACHE_DIR"
echo "$(date): Job completed successfully" > "$SKIP_MARKER""#,
            architecture
        );

        Ok(Step {
            id: Some("record_completion".to_string()),
            name: Some("Record job completion".to_string()),
            uses: None,
            run: Some(command),
            with: None,
            env: None,
            condition: None,
            working_directory: None,
            shell: Some("bash".to_string()),
            continue_on_error: None,
            timeout_minutes: None,
        })
    }
}

impl Default for JobSkipGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_generator_creation() {
        let generator = JobSkipGenerator::new();
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
            strategy: None,
            permissions: None,
            environment: None,
            concurrency: None,
            env: None,
        };

        let result = generator
            .generate_skip_steps(&config, &job, "amd64")
            .unwrap();
        assert!(result.is_none()); // No source_files defined
    }

    #[test]
    fn test_skip_steps_generation_with_source_files() {
        use std::collections::HashMap;

        let generator = JobSkipGenerator::new();

        // Create config with source file groups
        let mut source_file_groups = HashMap::new();
        source_file_groups.insert(
            "ruby".to_string(),
            vec![
                "**/*.rb".to_string(),
                "Gemfile*".to_string(),
                ".ruby-version".to_string(),
            ],
        );

        let config = Config {
            provider: "github-actions".to_string(),
            source_file_groups: Some(source_file_groups),
            ..Default::default()
        };

        // Create job with source_files
        let job = Job {
            image: "ruby:3.3".to_string(),
            source_files: Some(vec!["@ruby".to_string()]),
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
                strategy: None,
                permissions: None,
                environment: None,
                concurrency: None,
                env: None,
            }
        };

        let result = generator
            .generate_skip_steps(&config, &job, "amd64")
            .unwrap();
        assert!(result.is_some());

        let (hash_step, skip_step, completion_step) = result.unwrap();

        // Verify hash step
        assert_eq!(hash_step.id, Some("calculate_hash".to_string()));
        assert_eq!(
            hash_step.name,
            Some("Calculate source file hash".to_string())
        );
        assert!(hash_step.run.is_some());
        let hash_command = hash_step.run.unwrap();
        assert!(hash_command.contains("**/*.rb"));
        assert!(hash_command.contains("Gemfile*"));
        assert!(hash_command.contains(".ruby-version"));
        assert!(hash_command.contains("JOB_HASH"));

        // Verify skip check step
        assert_eq!(skip_step.id, Some("check_skip".to_string()));
        assert_eq!(
            skip_step.name,
            Some("Check if job should be skipped".to_string())
        );
        assert!(skip_step.run.is_some());
        let skip_command = skip_step.run.unwrap();
        assert!(skip_command.contains("/tmp/cigen_skip_cache"));
        assert!(skip_command.contains("job_${{ env.JOB_HASH }}_amd64"));
        assert!(skip_command.contains("exit 0"));

        // Verify completion step
        assert_eq!(completion_step.id, Some("record_completion".to_string()));
        assert_eq!(
            completion_step.name,
            Some("Record job completion".to_string())
        );
        assert!(completion_step.run.is_some());
        let completion_command = completion_step.run.unwrap();
        assert!(completion_command.contains("Recording successful completion"));
        assert!(completion_command.contains("job_${{ env.JOB_HASH }}_amd64"));
    }

    #[test]
    fn test_inline_source_files() {
        let generator = JobSkipGenerator::new();

        let config = Config {
            provider: "github-actions".to_string(),
            ..Default::default()
        };

        // Create job with inline source_files
        let job = Job {
            image: "node:20".to_string(),
            source_files: Some(vec!["src/**/*.ts".to_string(), "package.json".to_string()]),
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
                strategy: None,
                permissions: None,
                environment: None,
                concurrency: None,
                env: None,
            }
        };

        let result = generator
            .generate_skip_steps(&config, &job, "amd64")
            .unwrap();
        assert!(result.is_some());

        let (hash_step, _skip_step, _completion_step) = result.unwrap();

        let hash_command = hash_step.run.unwrap();
        assert!(hash_command.contains("src/**/*.ts"));
        assert!(hash_command.contains("package.json"));
    }
}
