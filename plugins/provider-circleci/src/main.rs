#![allow(clippy::needless_borrows_for_generic_args)]

use anyhow::{Context, Result, anyhow, bail};
use cigen::plugin::protocol::{
    self, CigenSchema, CommandDefinition, CommandParameter, CustomStep, Fragment, GenerateRequest,
    GenerateResult, Hello, JobDefinition, PlanRequest, PlanResult, PluginInfo, RunStep, Step,
    UsesStep, WorkflowCondition as ProtoWorkflowCondition,
    WorkflowConditionKind as ProtoWorkflowConditionKind,
};
use cigen::schema::WorkflowConfig;
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::Write;
use std::process::{Command, Stdio};

const PLUGIN_NAME: &str = "provider/circleci";
const PLUGIN_VERSION: &str = "0.1.0";
const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Default)]
struct ServiceDefinition {
    image: String,
    environment: Option<Mapping>,
}

#[derive(Clone, Debug, Default)]
struct SetupOptions {
    image: Option<String>,
    resource_class: Option<String>,
    compile_cigen: bool,
    compile_repository: Option<String>,
    compile_ref: Option<String>,
    compile_path: Option<String>,
    self_check: Option<SelfCheckOptions>,
}

#[derive(Clone, Debug, Default)]
struct SelfCheckOptions {
    enabled: bool,
    commit_on_diff: bool,
}

#[derive(Clone, Debug, Default)]
struct CheckoutConfig {
    shallow: bool,
    fetch_options: Option<String>,
    tag_fetch_options: Option<String>,
    clone_options: Option<String>,
    keyscan_github: bool,
    keyscan_gitlab: bool,
    keyscan_bitbucket: bool,
}

#[derive(Clone, Debug)]
struct WorkflowRunCondition {
    provider: Option<String>,
    kind: WorkflowRunConditionKind,
    key: Option<String>,
    equals_yaml: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkflowRunConditionKind {
    Parameter,
    Variable,
    Env,
    Expression,
}

#[derive(Clone, Debug)]
struct JobVariant<'a> {
    variant_name: String,
    job: &'a JobDefinition,
}

struct CircleciContext<'a> {
    schema: &'a CigenSchema,
    setup_options: SetupOptions,
    checkout: CheckoutConfig,
    services: HashMap<String, ServiceDefinition>,
    workflow_conditions: HashMap<String, Vec<WorkflowRunCondition>>,
    raw_config: Value,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cigen_provider_circleci=info".parse()?),
        )
        .with_target(false)
        .without_time()
        .init();

    tracing::info!("Starting {PLUGIN_NAME} v{PLUGIN_VERSION}");

    use cigen::plugin::framing::{receive_message, send_message};
    use std::io::{stdin, stdout};

    let mut stdin = stdin().lock();
    let mut stdout = stdout().lock();

    let hello: Hello = receive_message(&mut stdin).context("Failed to read Hello message")?;
    if hello.core_protocol != PROTOCOL_VERSION {
        anyhow::bail!(
            "Protocol version mismatch: core={}, plugin={PROTOCOL_VERSION}",
            hello.core_protocol
        );
    }

    let info = PluginInfo {
        name: PLUGIN_NAME.to_string(),
        version: PLUGIN_VERSION.to_string(),
        protocol: PROTOCOL_VERSION,
        capabilities: vec!["provider:circleci".to_string()],
        requires: vec![],
        conflicts_with: vec!["provider:*".to_string()],
        metadata: HashMap::new(),
    };

    send_message(&info, &mut stdout).context("Failed to send PluginInfo")?;
    tracing::info!("Handshake complete, entering message loop");

    loop {
        match receive_message::<PlanRequest, _>(&mut stdin) {
            Ok(_plan_request) => {
                let plan_result = PlanResult {
                    resources: vec![],
                    deps: vec![],
                    diagnostics: vec![],
                };
                send_message(&plan_result, &mut stdout).context("Failed to send PlanResult")?;
            }
            Err(err) => {
                tracing::warn!("Failed to receive PlanRequest: {err}");
                break;
            }
        }

        match receive_message::<GenerateRequest, _>(&mut stdin) {
            Ok(generate_request) => {
                tracing::info!(
                    "Received GenerateRequest for target: {}",
                    generate_request.target
                );

                let result = match generate_request.schema.as_ref() {
                    Some(schema) => match build_circleci_fragments(schema) {
                        Ok(fragments) => GenerateResult {
                            fragments,
                            diagnostics: vec![],
                        },
                        Err(error) => GenerateResult {
                            fragments: vec![],
                            diagnostics: vec![make_diagnostic("CIRCLECI_GENERATE_ERROR", error)],
                        },
                    },
                    None => GenerateResult {
                        fragments: vec![],
                        diagnostics: vec![make_diagnostic(
                            "CIRCLECI_GENERATE_ERROR",
                            anyhow!("GenerateRequest missing schema"),
                        )],
                    },
                };

                send_message(&result, &mut stdout).context("Failed to send GenerateResult")?;
            }
            Err(err) => {
                tracing::warn!("Exiting plugin loop: {err}");
                break;
            }
        }
    }

    Ok(())
}

fn build_circleci_fragments(schema: &CigenSchema) -> Result<Vec<Fragment>> {
    let raw_config: Value = serde_yaml::from_str(&schema.raw_config_yaml)
        .context("Failed to parse raw configuration from schema")?;

    let context = CircleciContext {
        schema,
        setup_options: extract_setup_options(&raw_config)?,
        checkout: extract_checkout_config(&raw_config),
        services: extract_services(&raw_config),
        workflow_conditions: extract_workflow_conditions(schema)?,
        raw_config,
    };

    let mut fragments = Vec::new();

    // 1. Generate .circleci/config.yml (setup workflow)
    let setup_config = generate_setup_config(&context)?;
    let setup_yaml = serde_yaml::to_string(&setup_config)?;

    if let Err(e) = validate_config_content(&setup_yaml) {
        bail!("Validation failed for setup config\nError: {}", e);
    }

    fragments.push(Fragment {
        path: ".circleci/config.yml".to_string(),
        content: setup_yaml,
        strategy: 0, // Replace
        format: "yaml".to_string(),
        order: 0,
    });

    // 2. Generate .circleci/main.yml (main workflow)
    let main_config = generate_main_config(&context)?;
    let main_yaml = serde_yaml::to_string(&main_config)?;
    validate_config_content(&main_yaml).context("Validation failed for main config")?;

    fragments.push(Fragment {
        path: ".circleci/main.yml".to_string(),
        content: main_yaml,
        strategy: 0, // Replace
        format: "yaml".to_string(),
        order: 0,
    });

    Ok(fragments)
}

fn validate_config_content(content: &str) -> Result<()> {
    tracing::info!("Starting validation for content length: {}", content.len());
    // Check for circleci CLI
    if Command::new("circleci")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        tracing::warn!("[DEBUG] circleci CLI not found, skipping validation");
        return Ok(()); // Skip if not installed
    }

    let mut child = Command::new("circleci")
        .arg("config")
        .arg("validate")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(content.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    tracing::info!("[DEBUG] CircleCI validate status: {:?}", output.status);

    if !output.stdout.is_empty() {
        tracing::warn!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        tracing::warn!("{}", String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        bail!("CircleCI validation failed");
    }

    Ok(())
}

fn extract_workflow_conditions(
    schema: &CigenSchema,
) -> Result<HashMap<String, Vec<WorkflowRunCondition>>> {
    let mut map = HashMap::new();
    for workflow in &schema.workflows {
        let mut conditions = Vec::new();
        for proto_cond in &workflow.run_when {
            conditions.push(WorkflowRunCondition::from_proto(proto_cond)?);
        }
        map.insert(workflow.id.clone(), conditions);
    }
    Ok(map)
}

fn generate_setup_config(context: &CircleciContext) -> Result<Value> {
    let mut root = Mapping::new();
    root.insert(Value::String("version".into()), Value::String("2.1".into()));
    root.insert(Value::String("setup".into()), Value::Bool(true));

    let mut parameters = if let Some(Value::Mapping(params)) =
        context.raw_config.get(&Value::String("parameters".into()))
    {
        params.clone()
    } else {
        Mapping::new()
    };

    if !parameters.contains_key(&Value::String("skip_cache".into())) {
        let mut def = Mapping::new();
        def.insert(
            Value::String("type".into()),
            Value::String("boolean".into()),
        );
        def.insert(Value::String("default".into()), Value::Bool(false));
        def.insert(
            Value::String("description".into()),
            Value::String("Disable job-status cache and rerun all jobs".into()),
        );
        parameters.insert(Value::String("skip_cache".into()), Value::Mapping(def));
    }
    root.insert(
        Value::String("parameters".into()),
        Value::Mapping(parameters),
    );

    let orbs = build_orbs_map();
    root.insert(Value::String("orbs".into()), Value::Mapping(orbs));

    let commands = build_commands_map(context)?;
    if !commands.is_empty() {
        root.insert(Value::String("commands".into()), Value::Mapping(commands));
    }

    let mut all_variants = Vec::new();
    let mut grouped_jobs: HashMap<String, Vec<&JobDefinition>> = HashMap::new();

    for job in &context.schema.jobs {
        let wf = if job.workflow.is_empty() {
            "ci"
        } else {
            &job.workflow
        };
        grouped_jobs.entry(wf.to_string()).or_default().push(job);
    }

    for (wf_id, _) in &grouped_jobs {
        let variants = collect_job_variants_for_workflow(context, wf_id)?;
        all_variants.extend(variants);
    }

    let jobs = {
        let mut jobs_map = Mapping::new();
        jobs_map.insert(
            Value::String("setup".into()),
            build_setup_job(context, "main", &all_variants)?,
        );
        jobs_map
    };
    root.insert(Value::String("jobs".into()), Value::Mapping(jobs));

    // Setup workflow
    let workflows = {
        let mut wf = Mapping::new();
        let mut setup_wf = Mapping::new();
        setup_wf.insert(
            Value::String("jobs".into()),
            Value::Sequence(vec![Value::String("setup".into())]),
        );
        wf.insert(Value::String("main".into()), Value::Mapping(setup_wf));
        wf
    };
    root.insert(Value::String("workflows".into()), Value::Mapping(workflows));

    Ok(Value::Mapping(root))
}

fn generate_main_config(context: &CircleciContext) -> Result<Value> {
    let mut root = Mapping::new();
    root.insert(Value::String("version".into()), Value::String("2.1".into()));

    if let Some(params) = context.raw_config.get(&Value::String("parameters".into())) {
        root.insert(Value::String("parameters".into()), params.clone());
    }

    let mut orbs = build_orbs_map();
    if let Some(Value::Mapping(user_orbs)) = context.raw_config.get(&Value::String("orbs".into())) {
        for (k, v) in user_orbs {
            orbs.insert(k.clone(), v.clone());
        }
    }
    root.insert(Value::String("orbs".into()), Value::Mapping(orbs));

    let commands = build_commands_map(context)?;
    if !commands.is_empty() {
        root.insert(Value::String("commands".into()), Value::Mapping(commands));
    }

    let mut grouped_jobs: HashMap<String, Vec<&JobDefinition>> = HashMap::new();
    for job in &context.schema.jobs {
        let wf = if job.workflow.is_empty() {
            "ci"
        } else {
            &job.workflow
        };
        grouped_jobs.entry(wf.to_string()).or_default().push(job);
    }

    let mut all_variants = Vec::new();
    let mut workflow_variants_map: HashMap<String, Vec<JobVariant>> = HashMap::new();

    for (wf_id, _) in &grouped_jobs {
        let variants = collect_job_variants_for_workflow(context, wf_id)?;
        workflow_variants_map.insert(wf_id.clone(), variants.clone());
        all_variants.extend(variants);
    }

    let mut jobs_map = Mapping::new();
    for variant in &all_variants {
        if let Some(job_def) = convert_job(variant, context)? {
            jobs_map.insert(Value::String(variant.variant_name.clone()), job_def);
        }
    }
    root.insert(Value::String("jobs".into()), Value::Mapping(jobs_map));

    let mut workflows_map = Mapping::new();
    for (wf_id, variants) in workflow_variants_map {
        let wf_def = build_workflow_def(context, &wf_id, &variants)?;
        workflows_map.insert(Value::String(wf_id), wf_def);
    }
    root.insert(
        Value::String("workflows".into()),
        Value::Mapping(workflows_map),
    );

    Ok(Value::Mapping(root))
}

fn build_orbs_map() -> Mapping {
    let mut orbs = Mapping::new();
    orbs.insert(
        Value::String("continuation".into()),
        Value::String("circleci/continuation@1.0.0".into()),
    );
    orbs
}

fn collect_job_variants_for_workflow<'a>(
    context: &'a CircleciContext<'a>,
    workflow_id: &str,
) -> Result<Vec<JobVariant<'a>>> {
    let mut variants = Vec::new();
    for job in &context.schema.jobs {
        let job_workflow = if job.workflow.is_empty() {
            "ci"
        } else {
            &job.workflow
        };
        if job_workflow != workflow_id {
            continue;
        }

        variants.push(JobVariant {
            variant_name: job.id.clone(),
            job,
        });
    }
    Ok(variants)
}

fn build_workflow_def(
    context: &CircleciContext,
    workflow_id: &str,
    variants: &[JobVariant],
) -> Result<Value> {
    let mut workflow_map = Mapping::new();

    if let Some(conditions) = context.workflow_conditions.get(workflow_id) {
        if let Some(when_value) = build_circleci_when(conditions)? {
            workflow_map.insert(Value::String("when".into()), when_value);
        }
    }

    workflow_map.insert(
        Value::String("jobs".into()),
        Value::Sequence(build_workflow_jobs_sequence(variants)),
    );

    Ok(Value::Mapping(workflow_map))
}

fn build_workflow_jobs_sequence(variants: &[JobVariant]) -> Vec<Value> {
    let mut entries = Vec::new();
    for variant in variants {
        let job = variant.job;

        // Check if job type is approval
        let is_approval = if let Some(extra_type) = job.extra.get("type") {
            if let Ok(val) = parse_yaml_value(extra_type) {
                val.as_str() == Some("approval")
            } else {
                false
            }
        } else {
            false
        };

        if is_approval {
            let mut job_config = Mapping::new();
            job_config.insert(
                Value::String("type".into()),
                Value::String("approval".into()),
            );

            if !job.needs.is_empty() {
                let mut requires = Vec::new();
                for need in &job.needs {
                    requires.push(Value::String(need.clone()));
                }
                job_config.insert(Value::String("requires".into()), Value::Sequence(requires));
            }

            let mut wrapper = Mapping::new();
            wrapper.insert(
                Value::String(variant.variant_name.clone()),
                Value::Mapping(job_config),
            );
            entries.push(Value::Mapping(wrapper));
            continue;
        }

        if job.needs.is_empty() {
            entries.push(Value::String(variant.variant_name.clone()));
        } else {
            let mut requires = Vec::new();
            for need in &job.needs {
                requires.push(Value::String(need.clone()));
            }
            let mut job_config = Mapping::new();
            job_config.insert(Value::String("requires".into()), Value::Sequence(requires));
            let mut wrapper = Mapping::new();
            wrapper.insert(
                Value::String(variant.variant_name.clone()),
                Value::Mapping(job_config),
            );
            entries.push(Value::Mapping(wrapper));
        }
    }
    entries
}

fn convert_job(variant: &JobVariant, context: &CircleciContext) -> Result<Option<Value>> {
    let job = variant.job;

    // Skip approval jobs in definition list (they only appear in workflows)
    if let Some(extra_type) = job.extra.get("type") {
        if let Ok(val) = parse_yaml_value(extra_type) {
            if val.as_str() == Some("approval") {
                return Ok(None);
            }
        }
    }

    let mut map = Mapping::new();

    let mut docker_entries = Vec::new();
    if !job.image.is_empty() {
        let mut image_map = Mapping::new();
        image_map.insert(
            Value::String("image".into()),
            Value::String(job.image.clone()),
        );
        docker_entries.push(Value::Mapping(image_map));
    }

    if !job.services.is_empty() {
        for service in &job.services {
            if let Some(definition) = context.services.get(service) {
                let mut service_map = Mapping::new();
                service_map.insert(
                    Value::String("image".into()),
                    Value::String(definition.image.clone()),
                );
                if let Some(env) = &definition.environment {
                    service_map.insert(
                        Value::String("environment".into()),
                        Value::Mapping(env.clone()),
                    );
                }
                docker_entries.push(Value::Mapping(service_map));
            } else {
                bail!(
                    "Unknown CircleCI service '{service}' referenced by job '{}'",
                    job.id
                );
            }
        }
    }

    if !docker_entries.is_empty() {
        map.insert(
            Value::String("docker".into()),
            Value::Sequence(docker_entries),
        );
    }

    let mut env_map = Mapping::new();
    if !job.env.is_empty() {
        for (key, value) in &job.env {
            env_map.insert(Value::String(key.clone()), Value::String(value.clone()));
        }
    }

    if !env_map.is_empty() {
        map.insert(Value::String("environment".into()), Value::Mapping(env_map));
    }

    if !job.runner.is_empty() {
        map.insert(
            Value::String("executor".into()),
            Value::String(job.runner.clone()),
        );
    }

    if let Some(resource_class_value) = job.extra.get("resource_class") {
        let val = parse_yaml_value(resource_class_value)?;
        map.insert(Value::String("resource_class".into()), val);
    }

    if let Some(parallelism_value) = job.extra.get("parallelism") {
        map.insert(
            Value::String("parallelism".into()),
            parse_yaml_value(parallelism_value)?,
        );
    }

    let mut steps = vec![build_checkout_invocation(&context.checkout)];
    if !job.source_files.is_empty() {
        steps.push(build_job_runtime_hash_step(job));
    }
    steps.extend(convert_steps_list(&job.steps)?);
    if !job.source_files.is_empty() {
        steps.push(build_job_completion_marker_step(job));
        steps.push(build_job_status_save_step(job));
    }
    map.insert(Value::String("steps".into()), Value::Sequence(steps));

    Ok(Some(Value::Mapping(map)))
}

// ... (rest of the file: helper functions build_checkout_invocation, etc. - copied from original, no substitution logic)

fn sanitize_job_name(name: &str) -> String {
    name.replace(['/', '\\'], "_")
}

fn convert_steps_list(steps: &[Step]) -> Result<Vec<Value>> {
    let mut converted = Vec::new();
    for step in steps {
        converted.push(convert_step(step)?);
    }
    Ok(converted)
}

fn convert_step(step: &Step) -> Result<Value> {
    match step
        .step_type
        .as_ref()
        .ok_or_else(|| anyhow!("missing step_type"))?
    {
        cigen::plugin::protocol::step::StepType::Run(RunStep {
            name,
            command,
            env,
            r#if,
        }) => {
            let mut run_map = Mapping::new();
            if !name.is_empty() {
                run_map.insert(Value::String("name".into()), Value::String(name.clone()));
            }
            run_map.insert(
                Value::String("command".into()),
                Value::String(command.clone()),
            );
            if !env.is_empty() {
                let mut env_map = Mapping::new();
                for (key, value) in env {
                    env_map.insert(Value::String(key.clone()), Value::String(value.clone()));
                }
                run_map.insert(Value::String("environment".into()), Value::Mapping(env_map));
            }
            if !r#if.is_empty() {
                run_map.insert(Value::String("if".into()), Value::String(r#if.clone()));
            }
            let mut wrapper = Mapping::new();
            wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
            Ok(Value::Mapping(wrapper))
        }
        cigen::plugin::protocol::step::StepType::Uses(UsesStep {
            module, with, r#if, ..
        }) => {
            let mut uses_map = Mapping::new();
            uses_map.insert(Value::String("uses".into()), Value::String(module.clone()));
            if !with.is_empty() {
                let mut with_map = Mapping::new();
                for (key, value) in with {
                    let val = parse_yaml_value(value)?;
                    with_map.insert(Value::String(key.clone()), val);
                }
                uses_map.insert(Value::String("with".into()), Value::Mapping(with_map));
            }
            if !r#if.is_empty() {
                uses_map.insert(Value::String("if".into()), Value::String(r#if.clone()));
            }
            Ok(Value::Mapping(uses_map))
        }
        cigen::plugin::protocol::step::StepType::RestoreCache(step) => {
            let mut restore_map = Mapping::new();
            if !step.name.is_empty() {
                restore_map.insert(
                    Value::String("name".into()),
                    Value::String(step.name.clone()),
                );
            }
            restore_map.insert(Value::String("key".into()), Value::String(step.key.clone()));
            if !step.keys.is_empty() {
                restore_map.insert(
                    Value::String("keys".into()),
                    Value::Sequence(step.keys.iter().map(|k| Value::String(k.clone())).collect()),
                );
            }
            if !step.restore_keys.is_empty() {
                restore_map.insert(
                    Value::String("restore_keys".into()),
                    Value::Sequence(
                        step.restore_keys
                            .iter()
                            .map(|k| Value::String(k.clone()))
                            .collect(),
                    ),
                );
            }
            if !step.extra.is_empty() {
                for (key, value) in &step.extra {
                    let val = parse_yaml_value(value)?;
                    restore_map.insert(Value::String(key.clone()), val);
                }
            }
            let mut wrapper = Mapping::new();
            wrapper.insert(
                Value::String("restore_cache".into()),
                Value::Mapping(restore_map),
            );
            Ok(Value::Mapping(wrapper))
        }
        cigen::plugin::protocol::step::StepType::SaveCache(step) => {
            let mut save_map = Mapping::new();
            if !step.name.is_empty() {
                save_map.insert(
                    Value::String("name".into()),
                    Value::String(step.name.clone()),
                );
            }
            save_map.insert(Value::String("key".into()), Value::String(step.key.clone()));
            if !step.paths.is_empty() {
                save_map.insert(
                    Value::String("paths".into()),
                    Value::Sequence(
                        step.paths
                            .iter()
                            .map(|p| Value::String(p.clone()))
                            .collect(),
                    ),
                );
            }
            if !step.extra.is_empty() {
                for (key, value) in &step.extra {
                    let val = parse_yaml_value(value)?;
                    save_map.insert(Value::String(key.clone()), val);
                }
            }
            let mut wrapper = Mapping::new();
            wrapper.insert(Value::String("save_cache".into()), Value::Mapping(save_map));
            Ok(Value::Mapping(wrapper))
        }
        cigen::plugin::protocol::step::StepType::Custom(CustomStep { yaml, .. }) => {
            let val = parse_yaml_value(yaml)?;
            Ok(val)
        }
    }
}

fn build_setup_job(
    context: &CircleciContext,
    workflow_id: &str,
    job_variants: &[JobVariant],
) -> Result<Value> {
    let mut job = Mapping::new();

    let image = context
        .setup_options
        .image
        .clone()
        .unwrap_or_else(|| "cimg/rust:1.76".to_string());

    let mut docker_entries = Vec::new();
    let mut docker_map = Mapping::new();
    docker_map.insert(Value::String("image".into()), Value::String(image));
    docker_entries.push(Value::Mapping(docker_map));
    job.insert(
        Value::String("docker".into()),
        Value::Sequence(docker_entries),
    );

    if let Some(resource_class) = &context.setup_options.resource_class {
        job.insert(
            Value::String("resource_class".into()),
            Value::String(resource_class.clone()),
        );
    }

    let mut steps = Vec::new();
    steps.push(build_checkout_invocation(&context.checkout));

    if context.setup_options.compile_cigen {
        steps.push(build_compile_cigen_step(&context.setup_options));
    }

    if let Some(self_check) = context
        .setup_options
        .self_check
        .as_ref()
        .filter(|cfg| cfg.enabled)
    {
        steps.push(build_self_check_step(self_check));
    }

    steps.push(build_skip_cache_parameter_step());
    steps.push(build_prepare_skip_list_step());

    for variant in job_variants {
        if variant.job.source_files.is_empty() {
            continue;
        }
        steps.push(build_job_hash_step(variant));
        steps.push(build_job_status_restore_step(variant));
        steps.push(build_skip_list_append_step(variant, workflow_id));
    }

    steps.push(build_generate_main_step(workflow_id));
    steps.push(build_continuation_step(&context.raw_config));

    job.insert(Value::String("steps".into()), Value::Sequence(steps));

    Ok(Value::Mapping(job))
}

fn build_compile_cigen_step(options: &SetupOptions) -> Value {
    let mut lines = Vec::new();
    lines.push("set -euo pipefail".to_string());

    if let Some(repo) = &options.compile_repository {
        let path = options
            .compile_path
            .clone()
            .unwrap_or_else(|| "/tmp/cigen".to_string());
        lines.push(format!("rm -rf {path}"));
        lines.push(format!("git clone {repo} {path}"));
        lines.push(format!("cd {path}"));
        if let Some(rev) = &options.compile_ref {
            lines.push(format!("git checkout {rev}"));
        }
        lines.push("cargo build --release".to_string());
        lines.push(format!(
            "echo \"export PATH=\\\"{path}/target/release:$PATH\\\"\" >> $BASH_ENV"
        ));
    } else {
        lines.push("cargo build --release".to_string());
        lines.push(
            "echo \"export PATH=\\\"$(pwd)/target/release:$PATH\\\"\" >> $BASH_ENV".to_string(),
        );
    }

    lines.push(String::new());
    let command = lines.join("\n");

    let mut run_map = Mapping::new();
    run_map.insert(
        Value::String("name".into()),
        Value::String("Compile cigen".into()),
    );
    run_map.insert(Value::String("command".into()), Value::String(command));

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
    Value::Mapping(wrapper)
}

fn build_self_check_step(options: &SelfCheckOptions) -> Value {
    let mut lines = vec![
        "set -euo pipefail".to_string(),
        "cp -f .circleci/config.yml .circleci/config.yml.bak".to_string(),
        "cigen generate".to_string(),
        "if ! diff -q .circleci/config.yml .circleci/config.yml.bak > /dev/null 2>&1; then"
            .to_string(),
    ];
    if options.commit_on_diff {
        lines.push("  git config user.email \"ci@cigen.dev\"".to_string());
        lines.push("  git config user.name \"CIGen\"".to_string());
        lines.push("  git add .circleci/config.yml".to_string());
        lines.push(
            "  git commit -m \"ci: update .circleci/config.yml from cigen\" || true".to_string(),
        );
        lines.push("  git push || true".to_string());
    }
    lines.extend([
        "  echo 'Detected config drift after regeneration'".to_string(),
        "  exit 1".to_string(),
        "fi".to_string(),
        String::new(),
    ]);
    let command = lines.join("\n");

    let mut run_map = Mapping::new();
    run_map.insert(
        Value::String("name".into()),
        Value::String("Self-check entrypoint".into()),
    );
    run_map.insert(Value::String("command".into()), Value::String(command));

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
    Value::Mapping(wrapper)
}

fn build_skip_cache_parameter_step() -> Value {
    let command = [
        "set -euo pipefail".to_string(),
        "if [ \"<< pipeline.parameters.skip_cache >>\" = \"true\" ]; then".to_string(),
        "  cigen generate main".to_string(),
        "  circleci step halt".to_string(),
        "fi".to_string(),
        String::new(),
    ]
    .join("\n");

    let mut run_map = Mapping::new();
    run_map.insert(
        Value::String("name".into()),
        Value::String("Handle skip_cache parameter".into()),
    );
    run_map.insert(Value::String("command".into()), Value::String(command));

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
    Value::Mapping(wrapper)
}

fn build_prepare_skip_list_step() -> Value {
    let command =
        "rm -rf /tmp/skip && mkdir -p /tmp/skip /tmp/cigen /tmp/cigen_job_exists\n".to_string();

    let mut run_map = Mapping::new();
    run_map.insert(
        Value::String("name".into()),
        Value::String("Prepare skip list".into()),
    );
    run_map.insert(Value::String("command".into()), Value::String(command));

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
    Value::Mapping(wrapper)
}

fn build_job_hash_step(variant: &JobVariant) -> Value {
    let command = [
        "set -euo pipefail".to_string(),
        "mkdir -p /tmp/cigen".to_string(),
        format!(
            "JOB_HASH=$(cigen hash --job {} --config .cigen | tr -d '\\r')",
            variant.job.id
        ),
        "printf '%s' \"$JOB_HASH\" > /tmp/cigen/job_hash".to_string(),
        "echo \"export JOB_HASH=$JOB_HASH\" >> $BASH_ENV".to_string(),
        format!(
            "echo 'Computed hash for {}: '$JOB_HASH",
            variant.variant_name
        ),
        String::new(),
    ]
    .join("\n");

    let mut run_map = Mapping::new();
    run_map.insert(
        Value::String("name".into()),
        Value::String(format!("Hash sources for {}", variant.variant_name)),
    );
    run_map.insert(Value::String("command".into()), Value::String(command));

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
    Value::Mapping(wrapper)
}

fn build_job_status_restore_step(variant: &JobVariant) -> Value {
    let mut restore_map = Mapping::new();
    restore_map.insert(
        Value::String("name".into()),
        Value::String(format!("Restore job status: {}", variant.variant_name)),
    );
    restore_map.insert(
        Value::String("keys".into()),
        Value::Sequence(vec![
            Value::String(job_status_cache_key(&variant.variant_name)),
            Value::String("linux-{{ checksum \"/etc/os-release\" }}-job_status-exists-v1-".into()),
        ]),
    );

    let mut wrapper = Mapping::new();
    wrapper.insert(
        Value::String("restore_cache".into()),
        Value::Mapping(restore_map),
    );
    Value::Mapping(wrapper)
}

fn job_status_cache_key(job_name: &str) -> String {
    format!(
        "linux-{{{{ checksum \"/etc/os-release\" }}}}-job_status-exists-v1-{job_name}-{{{{ checksum \"/tmp/cigen/job_hash\" }}}}"
    )
}

fn build_job_runtime_hash_step(job: &JobDefinition) -> Value {
    let command = [
        "set -euo pipefail".to_string(),
        "mkdir -p /tmp/cigen /tmp/cigen_job_exists".to_string(),
        format!(
            "JOB_HASH=$(cigen hash --job {} --config .cigen | tr -d '\\r')",
            job.id
        ),
        "printf '%s' \"$JOB_HASH\" > /tmp/cigen/job_hash".to_string(),
        "echo \"export JOB_HASH=$JOB_HASH\" >> $BASH_ENV".to_string(),
        "echo \"Computed job hash: $JOB_HASH\"".to_string(),
        String::new(),
    ]
    .join("\n");

    let mut run_map = Mapping::new();
    run_map.insert(
        Value::String("name".into()),
        Value::String("Compute job hash".into()),
    );
    run_map.insert(Value::String("command".into()), Value::String(command));

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
    Value::Mapping(wrapper)
}

fn build_job_completion_marker_step(job: &JobDefinition) -> Value {
    let command = [
        "set -euo pipefail".to_string(),
        "mkdir -p /tmp/cigen_job_exists".to_string(),
        "if [ -z \"${JOB_HASH:-}\" ]; then".to_string(),
        format!(
            "  JOB_HASH=$(cigen hash --job {} --config .cigen | tr -d '\\r')",
            job.id
        ),
        "fi".to_string(),
        "printf '%s' \"$JOB_HASH\" > /tmp/cigen/job_hash".to_string(),
        "touch \"/tmp/cigen_job_exists/done_${JOB_HASH}\"".to_string(),
        "echo \"Recorded job completion for $JOB_HASH\"".to_string(),
        String::new(),
    ]
    .join("\n");

    let mut run_map = Mapping::new();
    run_map.insert(
        Value::String("name".into()),
        Value::String("Record job completion".into()),
    );
    run_map.insert(Value::String("command".into()), Value::String(command));
    run_map.insert(
        Value::String("when".into()),
        Value::String("on_success".into()),
    );

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
    Value::Mapping(wrapper)
}

fn build_job_status_save_step(job: &JobDefinition) -> Value {
    let mut save_map = Mapping::new();
    save_map.insert(
        Value::String("name".into()),
        Value::String("Persist job status".into()),
    );
    save_map.insert(
        Value::String("key".into()),
        Value::String(job_status_cache_key(&job.id)),
    );
    save_map.insert(
        Value::String("paths".into()),
        Value::Sequence(vec![Value::String("/tmp/cigen_job_exists".into())]),
    );
    save_map.insert(
        Value::String("when".into()),
        Value::String("on_success".into()),
    );

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("save_cache".into()), Value::Mapping(save_map));
    Value::Mapping(wrapper)
}

fn build_skip_list_append_step(variant: &JobVariant, workflow_id: &str) -> Value {
    let skip_file = format!("/tmp/skip/{}.txt", workflow_id);
    let command = [
        "set -euo pipefail".to_string(),
        format!(
            "if [ -f '/tmp/cigen_job_exists/done_${{JOB_HASH}}' ]; then echo '{}' >> {}; fi",
            variant.variant_name, skip_file
        ),
        "rm -rf /tmp/cigen_job_exists".to_string(),
        String::new(),
    ]
    .join("\n");

    let mut run_map = Mapping::new();
    run_map.insert(
        Value::String("name".into()),
        Value::String(format!("Probe exists: {}", variant.variant_name)),
    );
    run_map.insert(Value::String("command".into()), Value::String(command));

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
    Value::Mapping(wrapper)
}

fn build_generate_main_step(workflow_id: &str) -> Value {
    let skip_file = format!("/tmp/skip/{}.txt", workflow_id);
    let command = format!(
        "set -euo pipefail\nif [ -s \"{skip}\" ]; then\n  CIGEN_SKIP_JOBS_FILE=\"{skip}\" cigen generate main\nelse\n  cigen generate main\nfi\n",
        skip = skip_file
    );

    let mut run_map = Mapping::new();
    run_map.insert(
        Value::String("name".into()),
        Value::String("Generate filtered main".into()),
    );
    run_map.insert(Value::String("command".into()), Value::String(command));

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
    Value::Mapping(wrapper)
}

fn build_continuation_step(raw_config: &Value) -> Value {
    let mut params = Mapping::new();
    params.insert(
        Value::String("configuration_path".into()),
        Value::String(".circleci/main.yml".into()),
    );

    let parameters = extract_parameters(raw_config);
    if !parameters.is_empty() {
        let mut json_parts = Vec::new();
        for (name, type_) in parameters {
            let val = if type_ == "string" || type_ == "enum" {
                format!("\"<< pipeline.parameters.{name} >>\"")
            } else {
                format!("<< pipeline.parameters.{name} >>")
            };
            json_parts.push(format!("\"{}\": {}", name, val));
        }
        let json_str = format!("{{ {} }}", json_parts.join(", "));
        params.insert(Value::String("parameters".into()), Value::String(json_str));
    }

    let mut wrapper = Mapping::new();
    wrapper.insert(
        Value::String("continuation/continue".into()),
        Value::Mapping(params),
    );
    Value::Mapping(wrapper)
}

fn extract_parameters(raw: &Value) -> Vec<(String, String)> {
    raw.as_mapping()
        .and_then(|map| map.get(&Value::String("parameters".into())))
        .and_then(Value::as_mapping)
        .map(|mapping| {
            mapping
                .iter()
                .filter_map(|(k, v)| {
                    let name = k.as_str()?.to_string();
                    let type_ = v
                        .as_mapping()
                        .and_then(|m| m.get(&Value::String("type".into())))
                        .and_then(|t| t.as_str())
                        .unwrap_or("string")
                        .to_string();
                    Some((name, type_))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn build_commands_map(context: &CircleciContext) -> Result<Mapping> {
    let mut commands = default_commands()?;

    for (name, command) in &context.schema.commands {
        let command_value = convert_command_definition(command)?;
        commands.insert(Value::String(name.clone()), command_value);
    }

    Ok(commands)
}

fn default_commands() -> Result<Mapping> {
    const DEFAULT_SHALLOW_CHECKOUT: &str = include_str!("shallow_checkout.yml");
    let defaults: Mapping = serde_yaml::from_str(DEFAULT_SHALLOW_CHECKOUT)
        .context("Failed to parse embedded shallow checkout command")?;
    Ok(defaults)
}

fn convert_command_definition(command: &CommandDefinition) -> Result<Value> {
    let mut map = Mapping::new();

    if !command.description.is_empty() {
        map.insert(
            Value::String("description".into()),
            Value::String(command.description.clone()),
        );
    }

    if !command.parameters.is_empty() {
        let mut params = Mapping::new();
        for (name, parameter) in &command.parameters {
            params.insert(
                Value::String(name.clone()),
                convert_command_parameter(parameter)?,
            );
        }
        map.insert(Value::String("parameters".into()), Value::Mapping(params));
    }

    let steps = convert_steps_list(&command.steps)?;
    map.insert(Value::String("steps".into()), Value::Sequence(steps));

    if !command.extra.is_empty() {
        for (key, value) in &command.extra {
            map.insert(Value::String(key.clone()), parse_yaml_value(value)?);
        }
    }

    Ok(Value::Mapping(map))
}

fn convert_command_parameter(parameter: &CommandParameter) -> Result<Value> {
    let mut map = Mapping::new();

    if !parameter.r#type.is_empty() {
        map.insert(
            Value::String("type".into()),
            Value::String(parameter.r#type.clone()),
        );
    }

    if !parameter.description.is_empty() {
        map.insert(
            Value::String("description".into()),
            Value::String(parameter.description.clone()),
        );
    }

    if !parameter.default_yaml.is_empty() {
        let default_value = parse_yaml_value(&parameter.default_yaml)?;
        map.insert(Value::String("default".into()), default_value);
    }

    if !parameter.extra.is_empty() {
        for (key, value) in &parameter.extra {
            map.insert(Value::String(key.clone()), parse_yaml_value(value)?);
        }
    }

    Ok(Value::Mapping(map))
}

fn parse_yaml_value(content: &str) -> Result<Value> {
    serde_yaml::from_str(content).with_context(|| format!("Failed to parse YAML: {content}"))
}

impl WorkflowRunCondition {
    fn from_proto(proto: &ProtoWorkflowCondition) -> Result<Self> {
        let kind = ProtoWorkflowConditionKind::try_from(proto.kind)
            .map_err(|_| anyhow!("Unknown workflow condition kind value: {}", proto.kind))?;

        Ok(Self {
            provider: if proto.provider.is_empty() {
                None
            } else {
                Some(proto.provider.clone())
            },
            kind: match kind {
                ProtoWorkflowConditionKind::Parameter => WorkflowRunConditionKind::Parameter,
                ProtoWorkflowConditionKind::Variable => WorkflowRunConditionKind::Variable,
                ProtoWorkflowConditionKind::Env => WorkflowRunConditionKind::Env,
                ProtoWorkflowConditionKind::Expression => WorkflowRunConditionKind::Expression,
                ProtoWorkflowConditionKind::Unspecified => {
                    bail!("Workflow condition kind unspecified")
                }
            },
            key: if proto.key.is_empty() {
                None
            } else {
                Some(proto.key.clone())
            },
            equals_yaml: if proto.equals_yaml.is_empty() {
                None
            } else {
                Some(proto.equals_yaml.clone())
            },
        })
    }
}

fn make_diagnostic(code: &str, error: anyhow::Error) -> cigen::plugin::protocol::Diagnostic {
    cigen::plugin::protocol::Diagnostic {
        level: cigen::plugin::protocol::diagnostic::Level::Error as i32,
        code: code.to_string(),
        title: "CircleCI generation failed".to_string(),
        message: error.to_string(),
        fix_hint: String::new(),
        loc: None,
    }
}
fn build_checkout_invocation(config: &CheckoutConfig) -> Value {
    if !config.shallow
        && config.fetch_options.is_none()
        && config.tag_fetch_options.is_none()
        && config.clone_options.is_none()
        && !config.keyscan_github
        && !config.keyscan_gitlab
        && !config.keyscan_bitbucket
    {
        return Value::String("checkout".into());
    }

    let mut params = Mapping::new();

    if let Some(clone) = &config.clone_options {
        params.insert(
            Value::String("clone_options".into()),
            Value::String(clone.clone()),
        );
    }

    if let Some(fetch) = &config.fetch_options {
        params.insert(
            Value::String("fetch_options".into()),
            Value::String(fetch.clone()),
        );
    }

    if let Some(tag_fetch) = &config.tag_fetch_options {
        params.insert(
            Value::String("tag_fetch_options".into()),
            Value::String(tag_fetch.clone()),
        );
    }

    if config.keyscan_github {
        params.insert(Value::String("keyscan_github".into()), Value::Bool(true));
    }
    if config.keyscan_gitlab {
        params.insert(Value::String("keyscan_gitlab".into()), Value::Bool(true));
    }
    if config.keyscan_bitbucket {
        params.insert(Value::String("keyscan_bitbucket".into()), Value::Bool(true));
    }

    let mut wrapper = Mapping::new();
    wrapper.insert(
        Value::String("cigen_shallow_checkout".into()),
        Value::Mapping(params),
    );
    Value::Mapping(wrapper)
}

fn extract_services(raw_config: &Value) -> HashMap<String, ServiceDefinition> {
    let mut services = HashMap::new();

    let Value::Mapping(root) = raw_config else {
        return services;
    };

    let Some(Value::Mapping(service_map)) = root.get(&Value::String("services".into())) else {
        return services;
    };

    for (key, value) in service_map {
        let Some(name) = key.as_str() else { continue };
        let Value::Mapping(definition) = value else {
            continue;
        };

        let Some(image_value) = definition.get(&Value::String("image".into())) else {
            continue;
        };

        let Some(image) = image_value.as_str() else {
            continue;
        };

        let environment = definition
            .get(&Value::String("environment".into()))
            .and_then(Value::as_mapping)
            .cloned();

        services.insert(
            name.to_string(),
            ServiceDefinition {
                image: image.to_string(),
                environment,
            },
        );
    }

    services
}

fn extract_setup_options(raw_config: &Value) -> Result<SetupOptions> {
    let mut options = SetupOptions::default();

    let Some(value) = raw_config
        .as_mapping()
        .and_then(|map| map.get(&Value::String("setup_options".into())))
    else {
        return Ok(options);
    };

    let Value::Mapping(map) = value else {
        bail!("setup_options must be a mapping")
    };

    if let Some(image) = map
        .get(&Value::String("image".into()))
        .and_then(Value::as_str)
    {
        options.image = Some(image.to_string());
    }

    if let Some(resource_class) = map
        .get(&Value::String("resource_class".into()))
        .and_then(Value::as_str)
    {
        options.resource_class = Some(resource_class.to_string());
    }

    if let Some(compile) = map
        .get(&Value::String("compile_cigen".into()))
        .and_then(Value::as_bool)
    {
        options.compile_cigen = compile;
    }

    if let Some(repo) = map
        .get(&Value::String("compile_repository".into()))
        .and_then(Value::as_str)
    {
        options.compile_repository = Some(repo.to_string());
    }

    if let Some(rev) = map
        .get(&Value::String("compile_ref".into()))
        .and_then(Value::as_str)
    {
        options.compile_ref = Some(rev.to_string());
    }

    if let Some(path) = map
        .get(&Value::String("compile_path".into()))
        .and_then(Value::as_str)
    {
        options.compile_path = Some(path.to_string());
    }

    if let Some(Value::Mapping(self_map)) = map.get(&Value::String("self_check".into())) {
        let enabled = self_map
            .get(&Value::String("enabled".into()))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let commit_on_diff = self_map
            .get(&Value::String("commit_on_diff".into()))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        options.self_check = Some(SelfCheckOptions {
            enabled,
            commit_on_diff,
        });
    }

    if options.compile_cigen
        && options.compile_repository.is_none()
        && options.compile_path.is_none()
        && options.compile_ref.is_none()
    {
        // Compiling from the current repository is acceptable, so no explicit repository is required.
    }

    Ok(options)
}

fn extract_checkout_config(raw_config: &Value) -> CheckoutConfig {
    let mut config = CheckoutConfig::default();

    let Some(value) = raw_config
        .as_mapping()
        .and_then(|map| map.get(&Value::String("checkout".into())))
    else {
        return config;
    };

    match value {
        Value::Bool(false) => {
            config.shallow = true;
        }
        Value::Mapping(map) => {
            if let Some(shallow) = map
                .get(&Value::String("shallow".into()))
                .and_then(Value::as_bool)
            {
                config.shallow = shallow;
            }

            if let Some(fetch_options) = map
                .get(&Value::String("fetch_options".into()))
                .and_then(Value::as_str)
            {
                config.fetch_options = Some(fetch_options.to_string());
            }

            if let Some(tag_fetch_options) = map
                .get(&Value::String("tag_fetch_options".into()))
                .and_then(Value::as_str)
            {
                config.tag_fetch_options = Some(tag_fetch_options.to_string());
            }

            if let Some(clone_options) = map
                .get(&Value::String("clone_options".into()))
                .and_then(Value::as_str)
            {
                config.clone_options = Some(clone_options.to_string());
            }

            if let Some(keyscan) = map.get(&Value::String("keyscan".into()))
                && let Value::Mapping(keyscan_map) = keyscan
            {
                if let Some(val) = keyscan_map
                    .get(&Value::String("github".into()))
                    .and_then(Value::as_bool)
                {
                    config.keyscan_github = val;
                }
                if let Some(val) = keyscan_map
                    .get(&Value::String("gitlab".into()))
                    .and_then(Value::as_bool)
                {
                    config.keyscan_gitlab = val;
                }
                if let Some(val) = keyscan_map
                    .get(&Value::String("bitbucket".into()))
                    .and_then(Value::as_bool)
                {
                    config.keyscan_bitbucket = val;
                }
            }
        }
        _ => {}
    }

    config
}

fn extract_mapping(raw_config: &Value, key: &str) -> Option<Mapping> {
    raw_config
        .as_mapping()
        .and_then(|map| map.get(&Value::String(key.into())))
        .and_then(Value::as_mapping)
        .cloned()
}

fn build_circleci_when(conditions: &[WorkflowRunCondition]) -> Result<Option<Value>> {
    let mut clauses = Vec::new();

    for condition in conditions {
        if let Some(provider) = condition.provider.as_deref()
            && provider != "circleci"
        {
            continue;
        }

        match condition.kind {
            WorkflowRunConditionKind::Parameter => {
                let key = condition
                    .key
                    .as_deref()
                    .ok_or_else(|| anyhow!("Workflow parameter condition missing key"))?;
                let equals_value = parse_condition_equals(&condition.equals_yaml)?;
                let mut equal_values = Vec::new();
                equal_values.push(equals_value);
                equal_values.push(Value::String(format!("<< pipeline.parameters.{key} >>")));
                let mut equal_map = Mapping::new();
                equal_map.insert(Value::String("equal".into()), Value::Sequence(equal_values));
                clauses.push(Value::Mapping(equal_map));
            }
            WorkflowRunConditionKind::Variable
            | WorkflowRunConditionKind::Env
            | WorkflowRunConditionKind::Expression => {
                bail!(
                    "Workflow condition type {:?} is not supported on CircleCI",
                    condition.kind
                );
            }
        }
    }

    if clauses.is_empty() {
        return Ok(None);
    }

    if clauses.len() == 1 {
        Ok(Some(clauses.remove(0)))
    } else {
        let mut and_map = Mapping::new();
        and_map.insert(Value::String("and".into()), Value::Sequence(clauses));
        Ok(Some(Value::Mapping(and_map)))
    }
}

fn parse_condition_equals(equals_yaml: &Option<String>) -> Result<Value> {
    if let Some(yaml) = equals_yaml {
        let value: Value = serde_yaml::from_str(yaml)
            .with_context(|| format!("Failed to parse workflow condition value: {yaml}"))?;
        Ok(value)
    } else {
        Ok(Value::Bool(true))
    }
}
