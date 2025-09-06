use crate::models::config::ServiceEnvironment;
use crate::models::{Config, Job};
use crate::providers::circleci::config::{
    CircleCICommand, CircleCIConfig, CircleCIDockerAuth, CircleCIDockerImage, CircleCIJob,
    CircleCIStep, CircleCIWorkflow, CircleCIWorkflowJob, CircleCIWorkflowJobDetails,
};
use crate::providers::circleci::docker_images;
use crate::providers::circleci::template_commands;
use crate::validation::steps::StepValidator;
use globwalk::GlobWalkerBuilder;
use miette::{IntoDiagnostic, Result};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;

// Embedded template for GitHub status patch job
#[allow(dead_code)]
const PATCH_APPROVAL_JOBS_TEMPLATE: &str = include_str!("templates/patch_approval_jobs_status.yml");

pub struct CircleCIGenerator;

impl Default for CircleCIGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CircleCIGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_workflow(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
        commands: &HashMap<String, crate::models::Command>,
        output_path: &Path,
    ) -> Result<()> {
        let circleci_config = self.build_config(config, workflow_name, jobs, commands)?;

        // Write the YAML output
        let yaml_content = serde_yaml::to_string(&circleci_config).into_diagnostic()?;

        let output_file = if let Some(filename) = &config.output_filename {
            output_path.join(filename)
        } else {
            output_path.join("config.yml")
        };

        fs::create_dir_all(output_path).into_diagnostic()?;
        fs::write(&output_file, yaml_content).into_diagnostic()?;

        // Validate the generated config with CircleCI CLI
        self.validate_config(&output_file)?;

        Ok(())
    }

    pub fn generate_all(
        &self,
        config: &Config,
        workflows: &HashMap<String, HashMap<String, Job>>,
        commands: &HashMap<String, crate::models::Command>,
        output_path: &Path,
    ) -> Result<()> {
        // First validate all jobs across all workflows
        let validator = StepValidator::new();
        for workflow_jobs in workflows.values() {
            validator.validate_step_references(workflow_jobs, commands, "circleci")?;
        }

        let mut circleci_config = CircleCIConfig::default();

        // Handle setup workflows (either explicitly set or dynamic workflows)
        if config.setup.unwrap_or(false) || config.dynamic.unwrap_or(false) {
            circleci_config.setup = Some(true);
        }

        // Add pipeline parameters if specified
        if let Some(parameters) = &config.parameters {
            circleci_config.parameters = Some(self.convert_parameters(parameters)?);
        }

        // Add orbs if specified
        if let Some(orbs) = &config.orbs {
            circleci_config.orbs = Some(orbs.clone());
        }

        // Process all workflows
        for (workflow_name, jobs) in workflows {
            // Augment jobs with docker_build if enabled
            let mut jobs_augmented = jobs.clone();
            if let Some(db) = &config.docker_build
                && db.enabled
            {
                self.augment_with_docker_build(config, &mut jobs_augmented)?;
            }

            let workflow_config = self.build_workflow(workflow_name, &jobs_augmented)?;
            circleci_config
                .workflows
                .insert(workflow_name.clone(), workflow_config);

            // Process all jobs in the workflow with architecture variants
            for (job_name, job_def) in &jobs_augmented {
                // Skip approval jobs (they are workflow-level only)
                if job_def.job_type.as_deref() == Some("approval") {
                    continue;
                }
                let architectures = job_def
                    .architectures
                    .clone()
                    .unwrap_or_else(|| vec!["amd64".to_string()]); // Default to amd64

                for arch in &architectures {
                    let variant_job_name = if architectures.len() > 1 {
                        format!("{}_{}", job_name, arch)
                    } else {
                        job_name.clone()
                    };

                    let circleci_job = self.convert_job_with_architecture(config, job_def, arch)?;
                    circleci_config.jobs.insert(variant_job_name, circleci_job);
                }
            }
        }

        // Add services as executors if any
        if let Some(services) = &config.services {
            circleci_config.executors = Some(self.build_executors(services)?);
        }

        // Scan for template command usage and add them to commands section
        let used_template_commands = self.scan_for_template_commands(&circleci_config);
        let used_user_commands = self.scan_for_user_commands(&circleci_config, commands);

        let mut all_commands = HashMap::new();

        // Add used template commands
        for cmd_name in used_template_commands {
            if let Some(cmd_def) = template_commands::get_template_command(&cmd_name) {
                let command: CircleCICommand = serde_yaml::from_value(cmd_def.clone())
                    .map_err(|e| miette::miette!("Failed to parse template command: {}", e))?;
                all_commands.insert(cmd_name, command);
            }
        }

        // Add used user commands
        for cmd_name in used_user_commands {
            if let Some(user_cmd) = commands.get(&cmd_name) {
                let command = self.convert_user_command(user_cmd)?;
                all_commands.insert(cmd_name, command);
            }
        }

        if !all_commands.is_empty() {
            circleci_config.commands = Some(all_commands);
        }

        // Write the YAML output
        let yaml_content = serde_yaml::to_string(&circleci_config).into_diagnostic()?;

        let output_file = if let Some(filename) = &config.output_filename {
            output_path.join(filename)
        } else {
            output_path.join("config.yml")
        };

        fs::create_dir_all(output_path).into_diagnostic()?;
        fs::write(&output_file, yaml_content).into_diagnostic()?;

        // Validate the generated config with CircleCI CLI
        self.validate_config(&output_file)?;

        Ok(())
    }

    pub fn build_config(
        &self,
        config: &Config,
        workflow_name: &str,
        jobs: &HashMap<String, Job>,
        commands: &HashMap<String, crate::models::Command>,
    ) -> Result<CircleCIConfig> {
        // Prepare a local, augmentable copy of jobs (for docker_build injection)
        let mut jobs_augmented: HashMap<String, Job> = jobs.clone();

        // First validate that all step references are valid
        let validator = StepValidator::new();
        validator.validate_step_references(&jobs_augmented, commands, "circleci")?;

        let mut circleci_config = CircleCIConfig::default();

        // Inject docker_build jobs and requires if enabled
        if let Some(db) = &config.docker_build
            && db.enabled
        {
            self.augment_with_docker_build(config, &mut jobs_augmented)?;
        }

        // Build workflow
        let workflow = self.build_workflow(workflow_name, &jobs_augmented)?;
        circleci_config
            .workflows
            .insert(workflow_name.to_string(), workflow);

        // Convert jobs with architecture variants
        for (job_name, job_def) in &jobs_augmented {
            // Skip approval jobs (they are workflow-level only)
            if job_def.job_type.as_deref() == Some("approval") {
                continue;
            }
            let architectures = job_def
                .architectures
                .clone()
                .unwrap_or_else(|| vec!["amd64".to_string()]); // Default to amd64

            for arch in &architectures {
                let variant_job_name = if architectures.len() > 1 {
                    format!("{}_{}", job_name, arch)
                } else {
                    job_name.clone()
                };

                let circleci_job = self.convert_job_with_architecture(config, job_def, arch)?;
                circleci_config.jobs.insert(variant_job_name, circleci_job);
            }
        }

        // Add services as executors if any
        if let Some(services) = &config.services {
            circleci_config.executors = Some(self.build_executors(services)?);
        }

        // Handle setup workflows (either explicitly set or dynamic workflows)
        if config.setup.unwrap_or(false) || config.dynamic.unwrap_or(false) {
            circleci_config.setup = Some(true);
        }

        // Add pipeline parameters if specified
        if let Some(parameters) = &config.parameters {
            circleci_config.parameters = Some(self.convert_parameters(parameters)?);
        }

        // Add orbs if specified (required for commands like slack/notify)
        if let Some(orbs) = &config.orbs {
            circleci_config.orbs = Some(orbs.clone());
        }

        // Scan for template command usage and add them to commands section
        let used_template_commands = self.scan_for_template_commands(&circleci_config);
        let used_user_commands = self.scan_for_user_commands(&circleci_config, commands);

        let mut all_commands = HashMap::new();

        // Add used template commands
        for cmd_name in used_template_commands {
            if let Some(cmd_def) = template_commands::get_template_command(&cmd_name) {
                let command: CircleCICommand = serde_yaml::from_value(cmd_def.clone())
                    .map_err(|e| miette::miette!("Failed to parse template command: {}", e))?;
                all_commands.insert(cmd_name, command);
            }
        }

        // Add used user commands
        for cmd_name in used_user_commands {
            if let Some(user_cmd) = commands.get(&cmd_name) {
                let command = self.convert_user_command(user_cmd)?;
                all_commands.insert(cmd_name, command);
            }
        }

        if !all_commands.is_empty() {
            circleci_config.commands = Some(all_commands);
        }

        Ok(circleci_config)
    }

    /// Compute a single base hash for docker_build from declared hash_sources across all images
    fn compute_docker_base_hash(&self, config: &Config) -> Result<Option<String>> {
        let db = match &config.docker_build {
            Some(db) if db.enabled => db,
            _ => return Ok(None),
        };

        // Determine project root (one level up if running in .cigen)
        let current_dir = std::env::current_dir().into_diagnostic()?;
        let base_dir = if current_dir.file_name() == Some(std::ffi::OsStr::new(".cigen")) {
            current_dir.parent().unwrap_or(&current_dir).to_path_buf()
        } else {
            current_dir
        };

        let mut files: HashSet<std::path::PathBuf> = HashSet::new();
        for img in &db.images {
            for pattern in &img.hash_sources {
                // Try to glob; if fails (e.g., plain file), just add directly if exists
                if let Ok(walker) = GlobWalkerBuilder::from_patterns(&base_dir, &[pattern.as_str()])
                    .follow_links(true)
                    .case_insensitive(false)
                    .build()
                {
                    for entry in walker.filter_map(Result::ok) {
                        let p = entry.path().to_path_buf();
                        if p.is_file() {
                            files.insert(p);
                        }
                    }
                } else {
                    let p = base_dir.join(pattern);
                    if p.is_file() {
                        files.insert(p);
                    } else if p.is_dir() {
                        // Collect all files recursively
                        for e in walkdir::WalkDir::new(&p).into_iter().filter_map(|e| e.ok()) {
                            let fp = e.path().to_path_buf();
                            if fp.is_file() {
                                files.insert(fp);
                            }
                        }
                    }
                }
            }
        }

        let mut files_vec: Vec<_> = files.into_iter().collect();
        files_vec.sort();

        let mut hasher = Sha256::new();
        for fp in files_vec {
            // include relative path for stability
            if let Ok(rel) = fp.strip_prefix(&base_dir) {
                hasher.update(rel.to_string_lossy().as_bytes());
            }
            if let Ok(bytes) = std::fs::read(&fp) {
                hasher.update(&bytes);
            }
        }
        let hash = format!("{:x}", hasher.finalize());
        Ok(Some(hash))
    }

    /// Augment workflow jobs to include docker build jobs and inject requires edges
    fn augment_with_docker_build(
        &self,
        config: &Config,
        jobs: &mut HashMap<String, Job>,
    ) -> Result<()> {
        let db = match &config.docker_build {
            Some(db) if db.enabled => db,
            _ => return Ok(()),
        };

        // Map images by name for quick lookup
        let mut images_by_name: HashMap<String, &crate::models::config::DockerBuildImage> =
            HashMap::new();
        for img in &db.images {
            images_by_name.insert(img.name.clone(), img);
        }

        // Collect required images and architectures across all jobs
        let mut required: HashMap<String, HashSet<String>> = HashMap::new();
        for (_job_name, job) in jobs.iter() {
            let image_ref = job.image.as_str();
            if image_ref.contains('/') || image_ref.contains(':') {
                continue; // already full reference
            }
            if images_by_name.contains_key(image_ref) {
                let archs = job
                    .architectures
                    .clone()
                    .unwrap_or_else(|| vec!["amd64".to_string()]);
                let entry = required.entry(image_ref.to_string()).or_default();
                for a in archs {
                    entry.insert(a);
                }
            }
        }

        if required.is_empty() {
            return Ok(());
        }

        // Ensure dependency chain images are included
        fn add_deps(
            img_name: &str,
            images: &HashMap<String, &crate::models::config::DockerBuildImage>,
            required: &mut HashMap<String, HashSet<String>>,
        ) {
            if let Some(img) = images.get(img_name) {
                for dep in &img.depends_on {
                    // Inherit required arch set; default to amd64 if none yet
                    let archs = required
                        .get(img_name)
                        .cloned()
                        .unwrap_or_else(|| HashSet::from(["amd64".to_string()]));
                    let entry = required.entry(dep.clone()).or_default();
                    for a in archs.iter() {
                        entry.insert(a.clone());
                    }
                    add_deps(dep, images, required);
                }
            }
        }

        let keys: Vec<String> = required.keys().cloned().collect();
        for k in keys {
            add_deps(&k, &images_by_name, &mut required);
        }

        // Precompute base hash once for all build jobs
        let base_hash = self
            .compute_docker_base_hash(config)?
            .ok_or_else(|| miette::miette!("Failed to compute docker base hash"))?;

        // Inject build jobs and requires
        for (img_name, archs) in required.iter() {
            // Create/merge a build job definition
            let build_job_name = format!("build_{}", img_name);
            let mut build_job = Job {
                image: "cimg/base:current".to_string(),
                architectures: Some(archs.iter().cloned().collect()),
                resource_class: None,
                source_files: None,
                parallelism: None,
                requires: None,
                cache: None,
                restore_cache: None,
                services: None,
                packages: None,
                steps: Some(vec![]),
                checkout: None,
                job_type: None,
            };

            // Add DAG dependencies (as build jobs)
            if let Some(img) = images_by_name.get(img_name)
                && !img.depends_on.is_empty()
            {
                let reqs = img
                    .depends_on
                    .iter()
                    .map(|d| format!("build_{}", d))
                    .collect::<Vec<_>>();
                build_job.requires = Some(crate::models::job::JobRequires::Multiple(reqs));
            }

            // Build/push steps and job-status cache
            let mut raw_steps: Vec<serde_yaml::Value> = Vec::new();

            // Determine backend for job-status
            let backend = config
                .caches
                .as_ref()
                .and_then(|c| c.job_status.as_ref())
                .map(|b| b.backend.as_str())
                .unwrap_or("native");

            match backend {
                "redis" => {
                    let (redis_url, _ttl) = if let Some(cfg) = config
                        .caches
                        .as_ref()
                        .and_then(|c| c.job_status.as_ref())
                        .and_then(|b| b.config.as_ref())
                    {
                        let url = cfg
                            .get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("redis://127.0.0.1:6379");
                        let ttl = cfg.get("ttl_seconds").and_then(|v| v.as_u64());
                        (url.to_string(), ttl)
                    } else {
                        ("redis://127.0.0.1:6379".to_string(), None)
                    };

                    // Prepare key env
                    raw_steps.push(serde_yaml::to_value(serde_yaml::Mapping::from_iter([
                        (
                            serde_yaml::Value::String("run".to_string()),
                            serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                                (
                                    serde_yaml::Value::String("name".to_string()),
                                    serde_yaml::Value::String("Prepare job status key".to_string()),
                                ),
                                (
                                    serde_yaml::Value::String("command".to_string()),
                                    serde_yaml::Value::String(format!(
                                        "export JOB_STATUS_KEY=\"job_status:${{CIRCLE_JOB}}-${{DOCKER_ARCH}}-{}\"",
                                        base_hash
                                    )),
                                ),
                            ])),
                        ),
                    ])).unwrap());

                    // Check and skip
                    raw_steps.push(serde_yaml::to_value(serde_yaml::Mapping::from_iter([
                        (
                            serde_yaml::Value::String("run".to_string()),
                            serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                                (
                                    serde_yaml::Value::String("name".to_string()),
                                    serde_yaml::Value::String("Check job status (redis)".to_string()),
                                ),
                                (
                                    serde_yaml::Value::String("command".to_string()),
                                    serde_yaml::Value::String(format!(
                                        "VAL=$(redis-cli -u {} GET \"$JOB_STATUS_KEY\" 2>/dev/null || true)\nif [ \"$VAL\" = \"done\" ]; then\n  echo 'Job already completed successfully for this base hash. Skipping...';\n  circleci step halt;\nfi",
                                        redis_url
                                    )),
                                ),
                            ])),
                        ),
                    ])).unwrap());

                    // setup_remote_docker (optionally with layer caching)
                    raw_steps.push(self.setup_remote_docker_step(config));

                    // docker login if pushing with auth
                    let push_enabled = images_by_name
                        .get(img_name)
                        .and_then(|i| i.push)
                        .unwrap_or(db.registry.push);
                    if push_enabled
                        && let Some(d) = &config.docker
                        && let Some(default_auth) = &d.default_auth
                        && let Some(auths) = &d.auth
                        && let Some(auth) = auths.get(default_auth)
                    {
                        raw_steps.push(
                            serde_yaml::to_value(serde_yaml::Mapping::from_iter([(
                                serde_yaml::Value::String("run".to_string()),
                                serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                                    (
                                        serde_yaml::Value::String("name".to_string()),
                                        serde_yaml::Value::String("Docker login".to_string()),
                                    ),
                                    (
                                        serde_yaml::Value::String("command".to_string()),
                                        serde_yaml::Value::String(format!(
                                            "echo '{}' | docker login -u '{}' --password-stdin",
                                            auth.password, auth.username
                                        )),
                                    ),
                                ])),
                            )]))
                            .unwrap(),
                        );
                    }
                }
                _ => {
                    // Native cache: write job-key, restore cache, skip if done
                    raw_steps.push(serde_yaml::to_value(serde_yaml::Mapping::from_iter([
                        (
                            serde_yaml::Value::String("run".to_string()),
                            serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                                (
                                    serde_yaml::Value::String("name".to_string()),
                                    serde_yaml::Value::String("Prepare job status key".to_string()),
                                ),
                                (
                                    serde_yaml::Value::String("command".to_string()),
                                    serde_yaml::Value::String(format!(
                                        "mkdir -p /tmp/cigen_job_status && echo \"${{CIRCLE_JOB}}-${{DOCKER_ARCH}}-{}\" > /tmp/cigen_job_status/job-key",
                                        base_hash
                                    )),
                                ),
                            ])),
                        ),
                    ])).unwrap());

                    raw_steps.push(serde_yaml::to_value(serde_yaml::Mapping::from_iter([
                        (
                            serde_yaml::Value::String("restore_cache".to_string()),
                            serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                                (
                                    serde_yaml::Value::String("keys".to_string()),
                                    serde_yaml::Value::Sequence(vec![
                                        serde_yaml::Value::String("job_status-v1-{{ checksum \"/tmp/cigen_job_status/job-key\" }}".to_string()),
                                        serde_yaml::Value::String("job_status-v1-".to_string()),
                                    ]),
                                ),
                            ])),
                        ),
                    ])).unwrap());

                    raw_steps.push(serde_yaml::to_value(serde_yaml::Mapping::from_iter([
                        (
                            serde_yaml::Value::String("run".to_string()),
                            serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                                (
                                    serde_yaml::Value::String("name".to_string()),
                                    serde_yaml::Value::String("Check if build should be skipped".to_string()),
                                ),
                                (
                                    serde_yaml::Value::String("command".to_string()),
                                    serde_yaml::Value::String(format!(
                                        "if [ -f '/tmp/cigen_job_status/done_{}' ]; then\n  echo 'Build already done for this base hash. Skipping...';\n  circleci step halt;\nfi",
                                        base_hash
                                    )),
                                ),
                            ])),
                        ),
                    ])).unwrap());

                    // setup_remote_docker (optionally with layer caching)
                    raw_steps.push(self.setup_remote_docker_step(config));
                }
            }

            // Build and push
            if let Some(img) = images_by_name.get(img_name) {
                let mut build_cmd = format!(
                    "docker build -f {} -t {}/{}:{}-${{DOCKER_ARCH}} {}",
                    img.dockerfile, db.registry.repo, img_name, base_hash, img.context
                );
                if let Some(args) = &img.build_args {
                    for (k, v) in args.iter() {
                        build_cmd.push_str(&format!(" --build-arg {}={} ", k, v));
                    }
                }

                raw_steps.push(
                    serde_yaml::to_value(serde_yaml::Mapping::from_iter([(
                        serde_yaml::Value::String("run".to_string()),
                        serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                            (
                                serde_yaml::Value::String("name".to_string()),
                                serde_yaml::Value::String("Build docker image".to_string()),
                            ),
                            (
                                serde_yaml::Value::String("command".to_string()),
                                serde_yaml::Value::String(build_cmd),
                            ),
                        ])),
                    )]))
                    .unwrap(),
                );

                let push_enabled = img.push.unwrap_or(db.registry.push);
                if push_enabled {
                    raw_steps.push(
                        serde_yaml::to_value(serde_yaml::Mapping::from_iter([(
                            serde_yaml::Value::String("run".to_string()),
                            serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                                (
                                    serde_yaml::Value::String("name".to_string()),
                                    serde_yaml::Value::String("Push docker image".to_string()),
                                ),
                                (
                                    serde_yaml::Value::String("command".to_string()),
                                    serde_yaml::Value::String(format!(
                                        "docker push {}/{}:{}-${{DOCKER_ARCH}}",
                                        db.registry.repo, img_name, base_hash
                                    )),
                                ),
                            ])),
                        )]))
                        .unwrap(),
                    );
                }

                // Record completion
                match backend {
                    "redis" => {
                        let (redis_url, ttl) = if let Some(cfg) = config
                            .caches
                            .as_ref()
                            .and_then(|c| c.job_status.as_ref())
                            .and_then(|b| b.config.as_ref())
                        {
                            let url = cfg
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("redis://127.0.0.1:6379");
                            let ttl = cfg.get("ttl_seconds").and_then(|v| v.as_u64());
                            (url.to_string(), ttl)
                        } else {
                            ("redis://127.0.0.1:6379".to_string(), None)
                        };

                        let set_cmd = if let Some(ttl_secs) = ttl {
                            format!(
                                "redis-cli -u {} SET \"job_status:${{CIRCLE_JOB}}-${{DOCKER_ARCH}}-{}\" done >/dev/null 2>&1 || true\nredis-cli -u {} EXPIRE \"job_status:${{CIRCLE_JOB}}-${{DOCKER_ARCH}}-{}\" {} >/dev/null 2>&1 || true",
                                redis_url, base_hash, redis_url, base_hash, ttl_secs
                            )
                        } else {
                            format!(
                                "redis-cli -u {} SET \"job_status:${{CIRCLE_JOB}}-${{DOCKER_ARCH}}-{}\" done >/dev/null 2>&1 || true",
                                redis_url, base_hash
                            )
                        };

                        raw_steps.push(
                            serde_yaml::to_value(serde_yaml::Mapping::from_iter([(
                                serde_yaml::Value::String("run".to_string()),
                                serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                                    (
                                        serde_yaml::Value::String("name".to_string()),
                                        serde_yaml::Value::String(
                                            "Record job completion (redis)".to_string(),
                                        ),
                                    ),
                                    (
                                        serde_yaml::Value::String("command".to_string()),
                                        serde_yaml::Value::String(set_cmd),
                                    ),
                                ])),
                            )]))
                            .unwrap(),
                        );
                    }
                    _ => {
                        raw_steps.push(serde_yaml::to_value(serde_yaml::Mapping::from_iter([
                            (
                                serde_yaml::Value::String("run".to_string()),
                                serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                                    (
                                        serde_yaml::Value::String("name".to_string()),
                                        serde_yaml::Value::String("Record job completion".to_string()),
                                    ),
                                    (
                                        serde_yaml::Value::String("command".to_string()),
                                        serde_yaml::Value::String(format!(
                                            "mkdir -p /tmp/cigen_job_status && touch '/tmp/cigen_job_status/done_{}'",
                                            base_hash
                                        )),
                                    ),
                                ])),
                            ),
                        ])).unwrap());

                        raw_steps.push(serde_yaml::to_value(serde_yaml::Mapping::from_iter([
                            (
                                serde_yaml::Value::String("save_cache".to_string()),
                                serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([
                                    (
                                        serde_yaml::Value::String("key".to_string()),
                                        serde_yaml::Value::String("job_status-v1-{{ checksum \"/tmp/cigen_job_status/job-key\" }}".to_string()),
                                    ),
                                    (
                                        serde_yaml::Value::String("paths".to_string()),
                                        serde_yaml::Value::Sequence(vec![serde_yaml::Value::String("/tmp/cigen_job_status".to_string())]),
                                    ),
                                ])),
                            ),
                        ])).unwrap());
                    }
                }
            }

            // Attach raw steps
            if let Some(steps_vec) = &mut build_job.steps {
                for s in raw_steps {
                    steps_vec.push(crate::models::job::Step(s));
                }
            }
            jobs.insert(build_job_name.clone(), build_job);
        }

        // Inject requires into jobs that reference these images
        for (_name, job) in jobs.iter_mut() {
            // Skip approval jobs; they don't run a container and shouldn't depend on build jobs
            if job.job_type.as_deref() == Some("approval") {
                continue;
            }
            let image_ref = job.image.clone();
            if image_ref.contains('/') || image_ref.contains(':') {
                continue;
            }
            if required.contains_key(&image_ref) {
                let build_name = format!("build_{}", image_ref);
                let mut reqs = job.required_jobs();
                if !reqs.contains(&build_name) {
                    reqs.push(build_name);
                }
                job.requires = Some(crate::models::job::JobRequires::Multiple(reqs));
            }
        }

        Ok(())
    }

    /// Synthesize a minimal setup workflow that generates continuation config and continues the pipeline
    pub fn generate_synthesized_setup(
        &self,
        config: &Config,
        output_path: &std::path::Path,
    ) -> Result<()> {
        use crate::providers::circleci::config as cc;
        use serde_yaml::{Mapping, Value};

        let mut circleci_config = cc::CircleCIConfig {
            setup: Some(true),
            ..Default::default()
        };

        // Include orbs if specified (continuation or others)
        if let Some(orbs) = &config.orbs {
            circleci_config.orbs = Some(orbs.clone());
        }

        // Build setup workflow with a single job
        let workflow_name = "setup".to_string();
        let mut workflow = cc::CircleCIWorkflow {
            when: None,
            unless: None,
            jobs: Vec::new(),
        };

        let job_name = "generate_config".to_string();
        workflow
            .jobs
            .push(cc::CircleCIWorkflowJob::Simple(job_name.clone()));
        circleci_config.workflows.insert(workflow_name, workflow);

        // Construct the job
        let mut job = cc::CircleCIJob {
            executor: None,
            docker: Some(vec![cc::CircleCIDockerImage {
                image: "cimg/ruby:3.3.5".to_string(),
                auth: None,
                name: None,
                entrypoint: None,
                command: None,
                user: None,
                environment: None,
            }]),
            machine: None,
            resource_class: Some("small".to_string()),
            working_directory: None,
            parallelism: None,
            environment: None,
            parameters: None,
            steps: Vec::new(),
        };

        // Step: checkout
        job.steps
            .push(cc::CircleCIStep::new(Value::String("checkout".to_string())));

        // Optional hook: set_package_versions_hash if present in user commands
        if let Some(cmds) = &config.parameters {
            let _ = cmds; // retain for future param-driven hooks
        }
        // We can't access user commands here; add a safe reference (CircleCI will error if missing)
        // To avoid validation errors, only add if config.vars contains a marker (future). Skipping for now.

        // Step: run cigen to generate continuation config (.circleci/main.yml)
        let mut run_gen = Mapping::new();
        let mut run_gen_details = Mapping::new();
        run_gen_details.insert(
            Value::String("name".to_string()),
            Value::String("Generate continuation config".to_string()),
        );
        run_gen_details.insert(
            Value::String("command".to_string()),
            Value::String("cigen generate".to_string()),
        );
        run_gen.insert(
            Value::String("run".to_string()),
            Value::Mapping(run_gen_details),
        );
        job.steps
            .push(cc::CircleCIStep::new(Value::Mapping(run_gen)));

        // Step: continue pipeline using jq --rawfile
        let mut run_cont = Mapping::new();
        let mut run_cont_details = Mapping::new();
        run_cont_details.insert(
            Value::String("name".to_string()),
            Value::String("Continue Pipeline".to_string()),
        );
        let cont_cmd = r#"
if [ -z "${CIRCLE_CONTINUATION_KEY}" ]; then
  echo "CIRCLE_CONTINUATION_KEY is required. Make sure setup workflows are enabled."
  exit 1
fi

CONFIG_PATH=".circleci/main.yml"
# Branch to specific continuation config based on pipeline parameters
if [ "${CIRCLE_PIPELINE_PARAMETERS_check_package_versions}" = "true" ]; then
  CONFIG_PATH=".circleci/package_updates_config.yml"
elif [ "${CIRCLE_PIPELINE_PARAMETERS_run_staging_postman_tests}" = "true" ]; then
  CONFIG_PATH=".circleci/staging_postman_tests_config.yml"
fi
if [ ! -f "$CONFIG_PATH" ]; then
  echo "Continuation config not found at $CONFIG_PATH" >&2
  exit 1
fi

jq -n \
  --arg continuation "$CIRCLE_CONTINUATION_KEY" \
  --rawfile config "$CONFIG_PATH" \
  '{"continuation-key": $continuation, "configuration": $config}' \
  > /tmp/continuation.json

cat /tmp/continuation.json

[[ $(curl \
  -o /dev/stderr \
  -w '%{http_code}' \
  -XPOST \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  --data "@/tmp/continuation.json" \
  "https://circleci.com/api/v2/pipeline/continue") -eq 200 ]]
"#
        .trim()
        .to_string();
        run_cont_details.insert(
            Value::String("command".to_string()),
            Value::String(cont_cmd),
        );
        run_cont.insert(
            Value::String("run".to_string()),
            Value::Mapping(run_cont_details),
        );
        job.steps
            .push(cc::CircleCIStep::new(Value::Mapping(run_cont)));

        circleci_config.jobs.insert(job_name, job);

        // Write YAML
        let yaml_content = serde_yaml::to_string(&circleci_config).into_diagnostic()?;
        let output_file = output_path.join("config.yml");
        std::fs::create_dir_all(output_path).into_diagnostic()?;
        std::fs::write(&output_file, yaml_content).into_diagnostic()?;

        // Validate setup config
        self.validate_config(&output_file)?;

        Ok(())
    }

    fn build_workflow(
        &self,
        _workflow_name: &str,
        jobs: &HashMap<String, Job>,
    ) -> Result<CircleCIWorkflow> {
        let mut workflow_jobs = Vec::new();

        for (job_name, job_def) in jobs {
            // Approval jobs are added only at workflow level (no separate job definition)
            if job_def.job_type.as_deref() == Some("approval") {
                let details = CircleCIWorkflowJobDetails {
                    requires: job_def.requires.as_ref().map(|r| r.to_vec()),
                    context: None,
                    filters: None,
                    matrix: None,
                    name: None,
                    job_type: Some("approval".to_string()),
                    pre_steps: None,
                    post_steps: None,
                };

                let mut job_map = HashMap::new();
                job_map.insert(job_name.clone(), details);
                workflow_jobs.push(CircleCIWorkflowJob::Detailed { job: job_map });
                continue;
            }
            // Generate architecture variants if architectures are specified
            let architectures = job_def
                .architectures
                .clone()
                .unwrap_or_else(|| vec!["amd64".to_string()]); // Default to amd64

            for arch in &architectures {
                let variant_job_name = if architectures.len() > 1 {
                    format!("{}_{}", job_name, arch)
                } else {
                    job_name.clone()
                };

                // Handle job dependencies with architecture variants
                if let Some(requires) = &job_def.requires {
                    let arch_requires = requires
                        .to_vec()
                        .into_iter()
                        .map(|req| {
                            // If the required job also has architectures, append architecture
                            if let Some(required_job) = jobs.get(&req)
                                && let Some(req_archs) = &required_job.architectures
                                && req_archs.len() > 1
                                && req_archs.contains(arch)
                            {
                                return format!("{}_{}", req, arch);
                            }
                            req
                        })
                        .collect();

                    let details = CircleCIWorkflowJobDetails {
                        requires: Some(arch_requires),
                        context: None,
                        filters: None,
                        matrix: None,
                        name: None,
                        job_type: None,
                        pre_steps: None,
                        post_steps: None,
                    };

                    let mut job_map = HashMap::new();
                    job_map.insert(variant_job_name, details);

                    workflow_jobs.push(CircleCIWorkflowJob::Detailed { job: job_map });
                } else {
                    workflow_jobs.push(CircleCIWorkflowJob::Simple(variant_job_name));
                }
            }
        }

        Ok(CircleCIWorkflow {
            when: None,
            unless: None,
            jobs: workflow_jobs,
        })
    }

    pub(crate) fn convert_job_with_architecture(
        &self,
        config: &Config,
        job: &Job,
        architecture: &str,
    ) -> Result<CircleCIJob> {
        let mut circleci_job = CircleCIJob {
            executor: None,
            docker: None,
            machine: None,
            resource_class: job.resource_class.clone(),
            working_directory: None,
            parallelism: job.parallelism,
            environment: None,
            parameters: None,
            steps: Vec::new(),
        };

        // Set environment variables including DOCKER_ARCH
        let mut environment = HashMap::new();
        environment.insert("DOCKER_ARCH".to_string(), architecture.to_string());
        circleci_job.environment = Some(environment);

        // Build Docker images with architecture awareness
        let mut docker_images =
            vec![self.build_docker_image_with_architecture(config, &job.image, architecture)?];

        // Add service containers
        if let Some(service_refs) = &job.services
            && let Some(services) = &config.services
        {
            for service_ref in service_refs {
                if let Some(service) = services.get(service_ref) {
                    docker_images.push(self.build_service_image(config, service)?);
                }
            }
        }

        circleci_job.docker = Some(docker_images);

        // Add checkout step based on configuration hierarchy (skip for approval jobs)
        if (job.steps.is_none() || !Self::is_approval_job(job))
            && let Some(checkout_step) = self.resolve_checkout_step(config, None, job)?
        {
            circleci_job.steps.push(CircleCIStep::new(checkout_step));
        }

        // Add skip logic if job has source_files defined (job-status cache)
        let has_skip_logic = if let Some(source_files) = &job.source_files {
            self.add_skip_check_initial_steps(
                &mut circleci_job,
                config,
                source_files,
                architecture,
            )?;

            // Decide backend for job-status cache
            let backend = config
                .caches
                .as_ref()
                .and_then(|c| c.job_status.as_ref())
                .map(|b| b.backend.as_str())
                .unwrap_or("native");

            match backend {
                "redis" => {
                    // Prepare key env
                    let mut key_step = serde_yaml::Mapping::new();
                    let mut key_run = serde_yaml::Mapping::new();
                    key_run.insert(
                        serde_yaml::Value::String("name".to_string()),
                        serde_yaml::Value::String("Prepare job status key".to_string()),
                    );
                    key_run.insert(
                        serde_yaml::Value::String("command".to_string()),
                        serde_yaml::Value::String(
                            "export JOB_STATUS_KEY=\"job_status:${CIRCLE_JOB}-${DOCKER_ARCH}-${JOB_HASH}\"".to_string(),
                        ),
                    );
                    key_step.insert(
                        serde_yaml::Value::String("run".to_string()),
                        serde_yaml::Value::Mapping(key_run),
                    );
                    circleci_job
                        .steps
                        .push(CircleCIStep::new(serde_yaml::Value::Mapping(key_step)));

                    // Read redis url/ttl from config if provided
                    let (redis_url, ttl) = if let Some(cfg) = config
                        .caches
                        .as_ref()
                        .and_then(|c| c.job_status.as_ref())
                        .and_then(|b| b.config.as_ref())
                    {
                        let url = cfg
                            .get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("redis://127.0.0.1:6379");
                        let ttl = cfg.get("ttl_seconds").and_then(|v| v.as_u64());
                        (url.to_string(), ttl)
                    } else {
                        ("redis://127.0.0.1:6379".to_string(), None)
                    };

                    // Skip if key exists
                    let mut check_step = serde_yaml::Mapping::new();
                    let mut check_run = serde_yaml::Mapping::new();
                    check_run.insert(
                        serde_yaml::Value::String("name".to_string()),
                        serde_yaml::Value::String("Check job status (redis)".to_string()),
                    );
                    let check_cmd = format!(
                        "VAL=$(redis-cli -u {} GET \"$JOB_STATUS_KEY\" 2>/dev/null || true)\nif [ \"$VAL\" = \"done\" ]; then\n  echo 'Job already completed successfully for this file hash. Skipping...';\n  circleci step halt;\nfi",
                        redis_url
                    );
                    check_run.insert(
                        serde_yaml::Value::String("command".to_string()),
                        serde_yaml::Value::String(check_cmd),
                    );
                    check_step.insert(
                        serde_yaml::Value::String("run".to_string()),
                        serde_yaml::Value::Mapping(check_run),
                    );
                    circleci_job
                        .steps
                        .push(CircleCIStep::new(serde_yaml::Value::Mapping(check_step)));

                    // After steps complete, record completion and optionally set TTL
                    let mut record_step = serde_yaml::Mapping::new();
                    let mut record_run = serde_yaml::Mapping::new();
                    record_run.insert(
                        serde_yaml::Value::String("name".to_string()),
                        serde_yaml::Value::String("Record job completion (redis)".to_string()),
                    );
                    let set_cmd = if let Some(ttl_secs) = ttl {
                        format!(
                            "redis-cli -u {} SET \"$JOB_STATUS_KEY\" done >/dev/null 2>&1 || true\nredis-cli -u {} EXPIRE \"$JOB_STATUS_KEY\" {} >/dev/null 2>&1 || true",
                            redis_url, redis_url, ttl_secs
                        )
                    } else {
                        format!(
                            "redis-cli -u {} SET \"$JOB_STATUS_KEY\" done >/dev/null 2>&1 || true",
                            redis_url
                        )
                    };
                    record_run.insert(
                        serde_yaml::Value::String("command".to_string()),
                        serde_yaml::Value::String(set_cmd),
                    );
                    record_step.insert(
                        serde_yaml::Value::String("run".to_string()),
                        serde_yaml::Value::Mapping(record_run),
                    );
                    circleci_job
                        .steps
                        .push(CircleCIStep::new(serde_yaml::Value::Mapping(record_step)));
                }
                _ => {
                    // Native CircleCI cache
                    // 1) Write job-key file (CIRCLE_JOB + arch + hash)
                    let mut key_step = serde_yaml::Mapping::new();
                    let mut key_run = serde_yaml::Mapping::new();
                    key_run.insert(
                        serde_yaml::Value::String("name".to_string()),
                        serde_yaml::Value::String("Prepare job status key".to_string()),
                    );
                    key_run.insert(
                        serde_yaml::Value::String("command".to_string()),
                        serde_yaml::Value::String(
                            "mkdir -p /tmp/cigen_job_status && echo \"${CIRCLE_JOB}-$(echo $DOCKER_ARCH)-${JOB_HASH}\" > /tmp/cigen_job_status/job-key".to_string(),
                        ),
                    );
                    key_step.insert(
                        serde_yaml::Value::String("run".to_string()),
                        serde_yaml::Value::Mapping(key_run),
                    );
                    circleci_job
                        .steps
                        .push(CircleCIStep::new(serde_yaml::Value::Mapping(key_step)));

                    // 2) Restore job status cache
                    let mut restore_cfg = serde_yaml::Mapping::new();
                    restore_cfg.insert(
                        serde_yaml::Value::String("keys".to_string()),
                        serde_yaml::Value::Sequence(vec![
                            serde_yaml::Value::String(
                                "job_status-v1-{{ checksum \"/tmp/cigen_job_status/job-key\" }}"
                                    .to_string(),
                            ),
                            serde_yaml::Value::String("job_status-v1-".to_string()),
                        ]),
                    );
                    let mut restore_step = serde_yaml::Mapping::new();
                    restore_step.insert(
                        serde_yaml::Value::String("restore_cache".to_string()),
                        serde_yaml::Value::Mapping(restore_cfg),
                    );
                    circleci_job
                        .steps
                        .push(CircleCIStep::new(serde_yaml::Value::Mapping(restore_step)));

                    // 3) Check and early exit if restored
                    let skip_check_step = self.build_skip_check_step(config, architecture)?;
                    circleci_job.steps.push(CircleCIStep::new(skip_check_step));
                }
            }

            true
        } else {
            false
        };

        // Add automatic cache restoration based on job.cache field
        // This implements convention-over-configuration: declaring a cache automatically injects restore steps
        if let Some(cache_defs) = &job.cache {
            for (cache_name, cache_def) in cache_defs {
                // Only restore caches that have restore enabled (default is true)
                if cache_def.restore {
                    let mut restore_step = serde_yaml::Mapping::new();
                    let mut restore_config = serde_yaml::Mapping::new();

                    // Build the cache key using the cache name from config's cache_definitions
                    let cache_key = if let Some(config_cache_defs) = &config.cache_definitions {
                        if let Some(cache_config) = config_cache_defs.get(cache_name) {
                            if let Some(key_template) = &cache_config.key {
                                // Use the key template from cache_definitions
                                // Replace {{ arch }} with the actual architecture
                                // Note: We keep {{ checksum(...) }} as-is for CircleCI to process
                                key_template.replace("{{ arch }}", architecture)
                            } else {
                                // No key template, use a reasonable default
                                format!(
                                    "{}-{}-{{{{ checksum \"Gemfile.lock\" }}}}",
                                    cache_name, architecture
                                )
                            }
                        } else {
                            // Cache not in definitions, use simple format
                            format!(
                                "{}-{}-{{{{ checksum \"cache_key\" }}}}",
                                cache_name, architecture
                            )
                        }
                    } else {
                        // No cache definitions at all
                        format!(
                            "{}-{}-{{{{ checksum \"cache_key\" }}}}",
                            cache_name, architecture
                        )
                    };

                    restore_config.insert(
                        serde_yaml::Value::String("keys".to_string()),
                        serde_yaml::Value::Sequence(vec![
                            serde_yaml::Value::String(cache_key.clone()),
                            serde_yaml::Value::String(format!("{}-{}-", cache_name, architecture)),
                        ]),
                    );

                    restore_step.insert(
                        serde_yaml::Value::String("restore_cache".to_string()),
                        serde_yaml::Value::Mapping(restore_config),
                    );

                    circleci_job
                        .steps
                        .push(CircleCIStep::new(serde_yaml::Value::Mapping(restore_step)));
                }
            }
        }

        // Add restore_cache steps if explicitly specified (legacy support)
        if let Some(restore_caches) = &job.restore_cache {
            for cache in restore_caches {
                let cache_step = match cache {
                    crate::models::job::CacheRestore::Simple(name) => {
                        // Simple cache restoration - check if cache_definitions has a key
                        let mut restore_step = serde_yaml::Mapping::new();
                        let mut restore_config = serde_yaml::Mapping::new();

                        // Generate cache key - check cache_definitions first
                        let cache_key = if let Some(config_cache_defs) = &config.cache_definitions {
                            if let Some(cache_config) = config_cache_defs.get(name) {
                                if let Some(key_template) = &cache_config.key {
                                    key_template.replace("{{ arch }}", architecture)
                                } else {
                                    format!(
                                        "{}-{}-{{{{ checksum \"cache_key\" }}}}",
                                        name, architecture
                                    )
                                }
                            } else {
                                format!(
                                    "{}-{}-{{{{ checksum \"cache_key\" }}}}",
                                    name, architecture
                                )
                            }
                        } else {
                            format!("{}-{}-{{{{ checksum \"cache_key\" }}}}", name, architecture)
                        };

                        restore_config.insert(
                            serde_yaml::Value::String("keys".to_string()),
                            serde_yaml::Value::Sequence(vec![
                                serde_yaml::Value::String(cache_key.clone()),
                                serde_yaml::Value::String(format!("{}-{}-", name, architecture)),
                            ]),
                        );

                        restore_step.insert(
                            serde_yaml::Value::String("restore_cache".to_string()),
                            serde_yaml::Value::Mapping(restore_config),
                        );

                        serde_yaml::Value::Mapping(restore_step)
                    }
                    crate::models::job::CacheRestore::Complex {
                        name,
                        dependency: _,
                    } => {
                        // Complex cache restoration with dependency flag
                        let mut restore_step = serde_yaml::Mapping::new();
                        let mut restore_config = serde_yaml::Mapping::new();

                        // Generate cache key - check cache_definitions first
                        let cache_key = if let Some(config_cache_defs) = &config.cache_definitions {
                            if let Some(cache_config) = config_cache_defs.get(name) {
                                if let Some(key_template) = &cache_config.key {
                                    key_template.replace("{{ arch }}", architecture)
                                } else {
                                    format!(
                                        "{}-{}-{{{{ checksum \"cache_key\" }}}}",
                                        name, architecture
                                    )
                                }
                            } else {
                                format!(
                                    "{}-{}-{{{{ checksum \"cache_key\" }}}}",
                                    name, architecture
                                )
                            }
                        } else {
                            format!("{}-{}-{{{{ checksum \"cache_key\" }}}}", name, architecture)
                        };

                        restore_config.insert(
                            serde_yaml::Value::String("keys".to_string()),
                            serde_yaml::Value::Sequence(vec![
                                serde_yaml::Value::String(cache_key.clone()),
                                serde_yaml::Value::String(format!("{}-{}-", name, architecture)),
                            ]),
                        );

                        restore_step.insert(
                            serde_yaml::Value::String("restore_cache".to_string()),
                            serde_yaml::Value::Mapping(restore_config),
                        );

                        serde_yaml::Value::Mapping(restore_step)
                    }
                };

                circleci_job.steps.push(CircleCIStep::new(cache_step));
            }
        }

        // Convert steps - just pass through the raw YAML
        if let Some(steps) = &job.steps {
            for step in steps {
                circleci_job.steps.push(CircleCIStep::new(step.0.clone()));
            }
        }

        // Add automatic cache saving based on job.cache field
        // This implements convention-over-configuration: declaring a cache automatically injects save steps
        if let Some(cache_defs) = &job.cache {
            for (cache_name, cache_def) in cache_defs {
                // Save all caches that are defined (paths are the value)
                if !cache_def.paths.is_empty() {
                    let mut save_step = serde_yaml::Mapping::new();
                    let mut save_config = serde_yaml::Mapping::new();

                    // Build the cache key - same as restore key
                    let cache_key = if let Some(config_cache_defs) = &config.cache_definitions {
                        if let Some(cache_config) = config_cache_defs.get(cache_name) {
                            if let Some(key_template) = &cache_config.key {
                                // Use the key template from cache_definitions
                                // Replace {{ arch }} with the actual architecture
                                // Note: We keep {{ checksum(...) }} as-is for CircleCI to process
                                key_template.replace("{{ arch }}", architecture)
                            } else {
                                // No key template, use a reasonable default
                                format!(
                                    "{}-{}-{{{{ checksum \"Gemfile.lock\" }}}}",
                                    cache_name, architecture
                                )
                            }
                        } else {
                            // Cache not in definitions, use simple format
                            format!(
                                "{}-{}-{{{{ checksum \"cache_key\" }}}}",
                                cache_name, architecture
                            )
                        }
                    } else {
                        // No cache definitions at all
                        format!(
                            "{}-{}-{{{{ checksum \"cache_key\" }}}}",
                            cache_name, architecture
                        )
                    };

                    save_config.insert(
                        serde_yaml::Value::String("key".to_string()),
                        serde_yaml::Value::String(cache_key),
                    );

                    save_config.insert(
                        serde_yaml::Value::String("paths".to_string()),
                        serde_yaml::Value::Sequence(
                            cache_def
                                .paths
                                .iter()
                                .map(|p| serde_yaml::Value::String(p.clone()))
                                .collect(),
                        ),
                    );

                    save_step.insert(
                        serde_yaml::Value::String("save_cache".to_string()),
                        serde_yaml::Value::Mapping(save_config),
                    );

                    circleci_job
                        .steps
                        .push(CircleCIStep::new(serde_yaml::Value::Mapping(save_step)));
                }
            }
        }

        // Add record completion step at the end if skip logic is enabled
        if has_skip_logic {
            self.add_record_completion_step(&mut circleci_job, architecture)?;
        }

        Ok(circleci_job)
    }

    fn is_approval_job(job: &Job) -> bool {
        // Consider any job with a single step mapping containing type: approval as an approval job
        if let Some(steps) = &job.steps
            && steps.len() == 1
            && let serde_yaml::Value::Mapping(map) = &steps[0].0
            && map.contains_key(serde_yaml::Value::String("type".to_string()))
            && let Some(val) = map.get(serde_yaml::Value::String("type".to_string()))
            && val.as_str() == Some("approval")
        {
            return true;
        }
        false
    }

    fn build_docker_image_with_architecture(
        &self,
        config: &Config,
        image: &str,
        architecture: &str,
    ) -> Result<CircleCIDockerImage> {
        // Resolve docker image reference, optionally to built tag
        let resolved_image = if let Some(db) = &config.docker_build {
            if db.enabled {
                let names: HashSet<_> = db.images.iter().map(|i| i.name.as_str()).collect();
                if !image.contains('/') && !image.contains(':') && names.contains(image) {
                    let base_hash = self
                        .compute_docker_base_hash(config)?
                        .ok_or_else(|| miette::miette!("Failed to compute docker base hash"))?;
                    format!(
                        "{}/{}:{}-{}",
                        db.registry.repo, image, base_hash, architecture
                    )
                } else {
                    docker_images::resolve_docker_image(image, Some(architecture), config)
                        .map_err(|e| miette::miette!("Failed to resolve docker image: {}", e))?
                }
            } else {
                docker_images::resolve_docker_image(image, Some(architecture), config)
                    .map_err(|e| miette::miette!("Failed to resolve docker image: {}", e))?
            }
        } else {
            docker_images::resolve_docker_image(image, Some(architecture), config)
                .map_err(|e| miette::miette!("Failed to resolve docker image: {}", e))?
        };

        let mut docker_image = CircleCIDockerImage {
            image: resolved_image,
            auth: None,
            name: None,
            entrypoint: None,
            command: None,
            user: None,
            environment: None,
        };

        // Add authentication if configured
        if let Some(docker_config) = &config.docker
            && let Some(default_auth) = &docker_config.default_auth
            && let Some(auth_configs) = &docker_config.auth
            && let Some(auth) = auth_configs.get(default_auth)
        {
            docker_image.auth = Some(CircleCIDockerAuth {
                username: auth.username.clone(),
                password: auth.password.clone(),
            });
        }

        Ok(docker_image)
    }

    fn build_service_image(
        &self,
        config: &Config,
        service: &crate::models::Service,
    ) -> Result<CircleCIDockerImage> {
        let mut docker_image = CircleCIDockerImage {
            image: service.image.clone(),
            auth: None,
            name: None,
            entrypoint: None,
            command: None,
            user: None,
            environment: None,
        };

        // Add environment variables
        if let Some(env) = &service.environment {
            docker_image.environment = Some(match env {
                ServiceEnvironment::Map(map) => map.clone(),
                ServiceEnvironment::Array(arr) => {
                    // Convert array format ["KEY=value"] to HashMap
                    let mut env_map = HashMap::new();
                    for entry in arr {
                        if let Some((key, value)) = entry.split_once('=') {
                            env_map.insert(key.to_string(), value.to_string());
                        }
                    }
                    env_map
                }
            });
        }

        // Add authentication if specified
        if let Some(auth_name) = &service.auth
            && let Some(docker_config) = &config.docker
            && let Some(auth_configs) = &docker_config.auth
            && let Some(auth) = auth_configs.get(auth_name)
        {
            docker_image.auth = Some(CircleCIDockerAuth {
                username: auth.username.clone(),
                password: auth.password.clone(),
            });
        }

        Ok(docker_image)
    }

    fn build_executors(
        &self,
        _services: &HashMap<String, crate::models::Service>,
    ) -> Result<HashMap<String, crate::providers::circleci::config::CircleCIExecutor>> {
        // Placeholder for executor building logic
        // This would create reusable executors from service definitions
        Ok(HashMap::new())
    }

    fn validate_config(&self, config_file: &Path) -> Result<()> {
        // Allow skipping CLI validation via env var (useful in offline/test environments)
        if std::env::var("CIGEN_SKIP_CIRCLECI_CLI")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            println!(
                "Skipping CircleCI CLI validation due to CIGEN_SKIP_CIRCLECI_CLI environment variable"
            );
            return Ok(());
        }

        // Check if CircleCI CLI is installed
        let cli_check = Command::new("circleci").arg("version").output();

        match cli_check {
            Ok(output) if output.status.success() => {
                // CLI is installed, run validation
                println!("Validating config with CircleCI CLI...");

                let validation_result = Command::new("circleci")
                    .arg("config")
                    .arg("validate")
                    .arg("-c")
                    .arg(config_file)
                    .current_dir(config_file.parent().unwrap().parent().unwrap())
                    .output()
                    .into_diagnostic()?;

                if validation_result.status.success() {
                    println!("✓ Config file is valid");
                } else {
                    let stderr = String::from_utf8_lossy(&validation_result.stderr);
                    eprintln!("✗ Config validation failed:\n{stderr}");
                    return Err(miette::miette!(
                        "CircleCI CLI validation failed for config file: {}\n\
                         Working directory: {}\n\
                         CircleCI CLI error: {}",
                        config_file.display(),
                        config_file.parent().unwrap().parent().unwrap().display(),
                        stderr
                    ));
                }
            }
            Ok(_) => {
                // CLI exists but version command failed
                println!("Warning: CircleCI CLI found but version check failed");
                self.print_install_instructions();
            }
            Err(_) => {
                // CLI not installed
                println!("CircleCI CLI not found - skipping validation");
                self.print_install_instructions();
            }
        }

        Ok(())
    }

    fn print_install_instructions(&self) {
        println!("\nTo enable config validation, install the CircleCI CLI:");
        println!("  brew install circleci");
        println!("  # or");
        println!(
            "  curl -fLSs https://raw.githubusercontent.com/CircleCI-Public/circleci-cli/main/install.sh | bash"
        );
        println!("  # or visit: https://circleci.com/docs/local-cli/");
    }

    fn setup_remote_docker_step(&self, config: &Config) -> serde_yaml::Value {
        if let Some(db) = &config.docker_build
            && db.layer_caching.unwrap_or(false)
        {
            let mut map = serde_yaml::Mapping::new();
            let mut inner = serde_yaml::Mapping::new();
            inner.insert(
                serde_yaml::Value::String("docker_layer_caching".to_string()),
                serde_yaml::Value::Bool(true),
            );
            map.insert(
                serde_yaml::Value::String("setup_remote_docker".to_string()),
                serde_yaml::Value::Mapping(inner),
            );
            return serde_yaml::Value::Mapping(map);
        }
        serde_yaml::Value::String("setup_remote_docker".to_string())
    }

    fn scan_for_template_commands(&self, config: &CircleCIConfig) -> HashSet<String> {
        let mut used_commands = HashSet::new();

        // Scan all jobs for template command usage
        for job in config.jobs.values() {
            for step in &job.steps {
                if let Some(step_type) = &step.step_type
                    && template_commands::is_template_command(step_type)
                {
                    used_commands.insert(step_type.clone());
                }
            }
        }

        used_commands
    }

    fn scan_for_user_commands(
        &self,
        config: &CircleCIConfig,
        available_commands: &HashMap<String, crate::models::Command>,
    ) -> HashSet<String> {
        let mut used_commands = HashSet::new();
        let mut to_scan = Vec::new();

        // Scan all jobs for user command usage
        for job in config.jobs.values() {
            for step in &job.steps {
                // Check if the raw value is a string that matches a command name
                if let serde_yaml::Value::String(cmd_name) = &step.raw
                    && available_commands.contains_key(cmd_name)
                {
                    to_scan.push(cmd_name.clone());
                }

                // Also check step_type for mapped commands
                if let Some(step_type) = &step.step_type
                    && available_commands.contains_key(step_type)
                {
                    to_scan.push(step_type.clone());
                }

                // Handle command references in raw YAML (e.g., mapped commands with parameters)
                Self::scan_yaml_for_commands(
                    &step.raw,
                    available_commands,
                    &HashSet::new(),
                    &mut to_scan,
                );
            }
        }

        // Recursively scan commands for other command dependencies
        while let Some(cmd_name) = to_scan.pop() {
            if used_commands.contains(&cmd_name) {
                continue;
            }
            used_commands.insert(cmd_name.clone());

            // Check if this command references other commands
            if let Some(cmd) = available_commands.get(&cmd_name) {
                for step in &cmd.steps {
                    match step {
                        crate::models::command::Step::CommandRef(ref_name) => {
                            if available_commands.contains_key(ref_name)
                                && !used_commands.contains(ref_name)
                            {
                                to_scan.push(ref_name.clone());
                            }
                        }
                        crate::models::command::Step::Raw(raw_value) => {
                            // Handle command references in raw YAML (e.g., mapped commands with parameters)
                            Self::scan_yaml_for_commands(
                                raw_value,
                                available_commands,
                                &used_commands,
                                &mut to_scan,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }

        used_commands
    }

    /// Recursively scan YAML for command references
    fn scan_yaml_for_commands(
        value: &serde_yaml::Value,
        available_commands: &HashMap<String, crate::models::Command>,
        used_commands: &HashSet<String>,
        to_scan: &mut Vec<String>,
    ) {
        match value {
            serde_yaml::Value::Mapping(map) => {
                for (key, val) in map {
                    // Check if the key is a command name
                    if let serde_yaml::Value::String(cmd_name) = key
                        && available_commands.contains_key(cmd_name)
                        && !used_commands.contains(cmd_name)
                    {
                        to_scan.push(cmd_name.clone());
                    }
                    // Recursively scan the value
                    Self::scan_yaml_for_commands(val, available_commands, used_commands, to_scan);
                }
            }
            serde_yaml::Value::Sequence(seq) => {
                for item in seq {
                    Self::scan_yaml_for_commands(item, available_commands, used_commands, to_scan);
                }
            }
            _ => {}
        }
    }

    fn convert_parameters(
        &self,
        parameters: &HashMap<String, crate::models::config::ParameterConfig>,
    ) -> Result<HashMap<String, crate::providers::circleci::config::CircleCIParameter>> {
        let mut circleci_params = HashMap::new();

        for (name, param) in parameters {
            let circleci_param = match param.param_type.as_str() {
                "boolean" => crate::providers::circleci::config::CircleCIParameter::Boolean {
                    param_type: param.param_type.clone(),
                    default: param.default.as_ref().and_then(|v| v.as_bool()),
                    description: param.description.clone(),
                },
                _ => crate::providers::circleci::config::CircleCIParameter::String {
                    param_type: param.param_type.clone(),
                    default: param
                        .default
                        .as_ref()
                        .and_then(|v| v.as_str().map(String::from)),
                    description: param.description.clone(),
                },
            };

            circleci_params.insert(name.clone(), circleci_param);
        }

        Ok(circleci_params)
    }

    fn convert_user_command(&self, user_cmd: &crate::models::Command) -> Result<CircleCICommand> {
        // Convert cigen Command to CircleCICommand
        let mut steps = Vec::new();

        for step in &user_cmd.steps {
            match step {
                crate::models::command::Step::Simple { name, run, when } => {
                    // Convert simple step to CircleCI run format
                    let mut step_map = serde_yaml::Mapping::new();
                    let mut run_details = serde_yaml::Mapping::new();

                    run_details.insert(
                        serde_yaml::Value::String("name".to_string()),
                        serde_yaml::Value::String(name.clone()),
                    );

                    run_details.insert(
                        serde_yaml::Value::String("command".to_string()),
                        serde_yaml::Value::String(run.clone()),
                    );

                    // Add when condition if present
                    if let Some(when_condition) = when {
                        run_details.insert(
                            serde_yaml::Value::String("when".to_string()),
                            serde_yaml::Value::String(when_condition.clone()),
                        );
                    }

                    step_map.insert(
                        serde_yaml::Value::String("run".to_string()),
                        serde_yaml::Value::Mapping(run_details),
                    );

                    steps.push(CircleCIStep::new(serde_yaml::Value::Mapping(step_map)));
                }
                crate::models::command::Step::CommandRef(cmd_ref) => {
                    // Convert command reference to string step
                    steps.push(CircleCIStep::new(serde_yaml::Value::String(
                        cmd_ref.clone(),
                    )));
                }
                crate::models::command::Step::Raw(raw_value) => {
                    // Use raw value directly
                    steps.push(CircleCIStep::new(raw_value.clone()));
                }
            }
        }

        Ok(CircleCICommand {
            description: Some(user_cmd.description.clone()),
            parameters: user_cmd.parameters.as_ref().map(|params| {
                params
                    .iter()
                    .map(|(k, v)| {
                        // Handle different parameter types based on param_type
                        let param = match v.param_type.as_str() {
                            "boolean" => {
                                crate::providers::circleci::config::CircleCIParameter::Boolean {
                                    param_type: v.param_type.clone(),
                                    description: v.description.clone(),
                                    default: v.default.as_ref().and_then(|d| d.as_bool()),
                                }
                            }
                            _ => crate::providers::circleci::config::CircleCIParameter::String {
                                param_type: v.param_type.clone(),
                                description: v.description.clone(),
                                default: v
                                    .default
                                    .as_ref()
                                    .and_then(|d| d.as_str().map(|s| s.to_string())),
                            },
                        };
                        (k.clone(), param)
                    })
                    .collect()
            }),
            steps,
        })
    }

    /// Add initial skip check steps for job-status cache (hash calculation and skip check)
    fn add_skip_check_initial_steps(
        &self,
        circleci_job: &mut CircleCIJob,
        config: &Config,
        source_files: &Vec<String>,
        architecture: &str,
    ) -> Result<()> {
        // Add hash calculation step
        let hash_step = self.build_hash_calculation_step(config, source_files)?;
        circleci_job.steps.push(CircleCIStep::new(hash_step));

        // Add skip check step
        let skip_check_step = self.build_skip_check_step(config, architecture)?;
        circleci_job.steps.push(CircleCIStep::new(skip_check_step));

        Ok(())
    }

    fn build_hash_calculation_step(
        &self,
        config: &Config,
        source_files: &Vec<String>,
    ) -> Result<serde_yaml::Value> {
        let source_file_groups = config
            .source_file_groups
            .as_ref()
            .ok_or_else(|| miette::miette!("source_file_groups not defined in config"))?;

        let mut all_patterns = Vec::new();

        for source_file in source_files {
            if let Some(group_name) = source_file.strip_prefix('@') {
                // Named group reference like "@ruby"

                let file_patterns = source_file_groups.get(group_name).ok_or_else(|| {
                    miette::miette!(
                        "source_files group '{}' not found in source_file_groups",
                        group_name
                    )
                })?;

                all_patterns.extend(file_patterns.clone());
            } else {
                // Inline pattern like "scripts/**/*"
                all_patterns.push(source_file.clone());
            }
        }

        // Build find commands for all file patterns
        let mut find_commands = Vec::new();
        for pattern in all_patterns {
            if pattern.starts_with('(') && pattern.ends_with(')') {
                // Reference to another group like "(rails)"
                let referenced_group = &pattern[1..pattern.len() - 1];
                if let Some(referenced_patterns) = source_file_groups.get(referenced_group) {
                    for ref_pattern in referenced_patterns {
                        if ref_pattern.ends_with('/') {
                            find_commands
                                .push(format!("find {} -type f 2>/dev/null || true", ref_pattern));
                        } else {
                            find_commands.push(format!(
                                "[ -f {} ] && echo {} || true",
                                ref_pattern, ref_pattern
                            ));
                        }
                    }
                }
            } else if pattern.ends_with('/') {
                // Directory pattern
                find_commands.push(format!("find {} -type f 2>/dev/null || true", pattern));
            } else {
                // File pattern
                find_commands.push(format!("[ -f {} ] && echo {} || true", pattern, pattern));
            }
        }

        let command = format!(
            r#"
echo "Calculating hash for source files..."
TEMP_HASH_FILE="/tmp/source_files_for_hash"
rm -f "$TEMP_HASH_FILE"

{}

if [ -f "$TEMP_HASH_FILE" ]; then
    export JOB_HASH=$(sort "$TEMP_HASH_FILE" | xargs sha256sum | sha256sum | cut -d' ' -f1)
    echo "Hash calculated: $JOB_HASH"
else
    export JOB_HASH="empty"
    echo "No source files found, using empty hash"
fi
            "#,
            find_commands
                .iter()
                .map(|cmd| format!("{} >> \"$TEMP_HASH_FILE\"", cmd))
                .collect::<Vec<_>>()
                .join("\n")
        )
        .trim()
        .to_string();

        let mut step = serde_yaml::Mapping::new();
        let mut run_details = serde_yaml::Mapping::new();
        run_details.insert(
            serde_yaml::Value::String("name".to_string()),
            serde_yaml::Value::String("Calculate source file hash".to_string()),
        );
        run_details.insert(
            serde_yaml::Value::String("command".to_string()),
            serde_yaml::Value::String(command),
        );

        step.insert(
            serde_yaml::Value::String("run".to_string()),
            serde_yaml::Value::Mapping(run_details),
        );

        Ok(serde_yaml::Value::Mapping(step))
    }

    fn build_skip_check_step(
        &self,
        _config: &Config,
        _architecture: &str,
    ) -> Result<serde_yaml::Value> {
        let command = r#"
if [ -f "/tmp/cigen_job_status/done_${JOB_HASH}" ]; then
    echo "Job already completed successfully for this file hash. Skipping..."
    circleci step halt
else
    echo "No previous successful run found. Proceeding with job..."
    mkdir -p /tmp/cigen_job_status
fi
        "#
        .trim()
        .to_string();

        let mut step = serde_yaml::Mapping::new();
        let mut run_details = serde_yaml::Mapping::new();
        run_details.insert(
            serde_yaml::Value::String("name".to_string()),
            serde_yaml::Value::String("Check if job should be skipped".to_string()),
        );
        run_details.insert(
            serde_yaml::Value::String("command".to_string()),
            serde_yaml::Value::String(command),
        );

        step.insert(
            serde_yaml::Value::String("run".to_string()),
            serde_yaml::Value::Mapping(run_details),
        );

        Ok(serde_yaml::Value::Mapping(step))
    }

    fn add_record_completion_step(
        &self,
        circleci_job: &mut CircleCIJob,
        _architecture: &str,
    ) -> Result<()> {
        let command = r#"
echo "Recording successful completion for hash ${JOB_HASH}"
touch "/tmp/cigen_job_status/done_${JOB_HASH}"
        "#
        .trim()
        .to_string();

        let mut step = serde_yaml::Mapping::new();
        let mut run_details = serde_yaml::Mapping::new();
        run_details.insert(
            serde_yaml::Value::String("name".to_string()),
            serde_yaml::Value::String("Record job completion".to_string()),
        );
        run_details.insert(
            serde_yaml::Value::String("command".to_string()),
            serde_yaml::Value::String(command),
        );

        step.insert(
            serde_yaml::Value::String("run".to_string()),
            serde_yaml::Value::Mapping(run_details),
        );

        circleci_job
            .steps
            .push(CircleCIStep::new(serde_yaml::Value::Mapping(step)));

        // Save job status cache
        let mut save_cfg = serde_yaml::Mapping::new();
        save_cfg.insert(
            serde_yaml::Value::String("key".to_string()),
            serde_yaml::Value::String(
                "job_status-v1-{{ checksum \"/tmp/cigen_job_status/job-key\" }}".to_string(),
            ),
        );
        save_cfg.insert(
            serde_yaml::Value::String("paths".to_string()),
            serde_yaml::Value::Sequence(vec![serde_yaml::Value::String(
                "/tmp/cigen_job_status".to_string(),
            )]),
        );
        let mut save_step = serde_yaml::Mapping::new();
        save_step.insert(
            serde_yaml::Value::String("save_cache".to_string()),
            serde_yaml::Value::Mapping(save_cfg),
        );
        circleci_job
            .steps
            .push(CircleCIStep::new(serde_yaml::Value::Mapping(save_step)));

        Ok(())
    }

    /// Resolve checkout configuration based on hierarchy (job > workflow > global > default)
    fn resolve_checkout_step(
        &self,
        config: &Config,
        workflow_config: Option<&crate::models::config::WorkflowConfig>,
        job: &Job,
    ) -> Result<Option<serde_yaml::Value>> {
        use crate::models::config::{CheckoutConfig, CheckoutSetting};

        // Resolve checkout config with hierarchy: job > workflow > global > default
        let setting = job
            .checkout
            .as_ref()
            .or_else(|| workflow_config?.checkout.as_ref())
            .or(config.checkout.as_ref())
            .cloned();

        // Interpret setting with defaults
        let checkout_config = match setting {
            Some(CheckoutSetting::Bool(b)) => {
                if !b {
                    return Ok(None);
                }
                CheckoutConfig {
                    shallow: true,
                    clone_options: None,
                    fetch_options: None,
                    tag_fetch_options: None,
                    keyscan: None,
                    path: None,
                }
            }
            Some(CheckoutSetting::Config(cfg)) => cfg,
            None => CheckoutConfig {
                shallow: true,
                clone_options: None,
                fetch_options: None,
                tag_fetch_options: None,
                keyscan: None,
                path: None,
            },
        };

        if checkout_config.shallow {
            // Use our vendored shallow checkout command
            let mut shallow_checkout = serde_yaml::Mapping::new();

            // Add parameters if specified
            if let Some(clone_options) = &checkout_config.clone_options {
                shallow_checkout.insert(
                    serde_yaml::Value::String("clone_options".to_string()),
                    serde_yaml::Value::String(clone_options.clone()),
                );
            }
            if let Some(fetch_options) = &checkout_config.fetch_options {
                shallow_checkout.insert(
                    serde_yaml::Value::String("fetch_options".to_string()),
                    serde_yaml::Value::String(fetch_options.clone()),
                );
            }
            if let Some(tag_fetch_options) = &checkout_config.tag_fetch_options {
                shallow_checkout.insert(
                    serde_yaml::Value::String("tag_fetch_options".to_string()),
                    serde_yaml::Value::String(tag_fetch_options.clone()),
                );
            }
            if let Some(keyscan) = &checkout_config.keyscan {
                if keyscan.get("github").unwrap_or(&false) == &true {
                    shallow_checkout.insert(
                        serde_yaml::Value::String("keyscan_github".to_string()),
                        serde_yaml::Value::Bool(true),
                    );
                }
                if keyscan.get("gitlab").unwrap_or(&false) == &true {
                    shallow_checkout.insert(
                        serde_yaml::Value::String("keyscan_gitlab".to_string()),
                        serde_yaml::Value::Bool(true),
                    );
                }
                if keyscan.get("bitbucket").unwrap_or(&false) == &true {
                    shallow_checkout.insert(
                        serde_yaml::Value::String("keyscan_bitbucket".to_string()),
                        serde_yaml::Value::Bool(true),
                    );
                }
            }
            if let Some(path) = &checkout_config.path {
                shallow_checkout.insert(
                    serde_yaml::Value::String("path".to_string()),
                    serde_yaml::Value::String(path.clone()),
                );
            }

            if shallow_checkout.is_empty() {
                // Simple command with no parameters
                Ok(Some(serde_yaml::Value::String(
                    "cigen_shallow_checkout".to_string(),
                )))
            } else {
                // Command with parameters
                let mut shallow_step = serde_yaml::Mapping::new();
                shallow_step.insert(
                    serde_yaml::Value::String("cigen_shallow_checkout".to_string()),
                    serde_yaml::Value::Mapping(shallow_checkout),
                );
                Ok(Some(serde_yaml::Value::Mapping(shallow_step)))
            }
        } else {
            // Use standard CircleCI checkout
            let mut checkout_step = serde_yaml::Mapping::new();
            let mut checkout_params = serde_yaml::Mapping::new();

            if let Some(path) = &checkout_config.path {
                checkout_params.insert(
                    serde_yaml::Value::String("path".to_string()),
                    serde_yaml::Value::String(path.clone()),
                );
            }

            if checkout_params.is_empty() {
                checkout_step.insert(
                    serde_yaml::Value::String("checkout".to_string()),
                    serde_yaml::Value::Null,
                );
            } else {
                checkout_step.insert(
                    serde_yaml::Value::String("checkout".to_string()),
                    serde_yaml::Value::Mapping(checkout_params),
                );
            }

            Ok(Some(serde_yaml::Value::Mapping(checkout_step)))
        }
    }
}
