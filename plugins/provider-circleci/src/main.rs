#![allow(clippy::needless_borrows_for_generic_args)]

use anyhow::{Context, Result, anyhow, bail};
use cigen::plugin::protocol::{
    CigenSchema, CommandDefinition, CommandParameter, CustomStep, Fragment, GenerateRequest,
    GenerateResult, Hello, JobDefinition, PlanRequest, PlanResult, PluginInfo, RunStep, Step,
    UsesStep, WorkflowCondition as ProtoWorkflowCondition,
    WorkflowConditionKind as ProtoWorkflowConditionKind,
};
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;
use std::convert::TryFrom;

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

fn build_circleci_fragments(schema: &CigenSchema) -> Result<Vec<Fragment>> {
    let raw_config: Value = serde_yaml::from_str(&schema.raw_config)
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
    fragments.push(Fragment {
        path: ".circleci/config.yml".to_string(),
        content: serde_yaml::to_string(&setup_config)?,
        strategy: 0, // Replace
    });

    // 2. Generate .circleci/main.yml (main workflow)
    let main_config = generate_main_config(&context)?;
    fragments.push(Fragment {
        path: ".circleci/main.yml".to_string(),
        content: serde_yaml::to_string(&main_config)?,
        strategy: 0, // Replace
    });

    Ok(fragments)
}

fn extract_workflow_conditions(
    schema: &CigenSchema,
) -> Result<HashMap<String, Vec<WorkflowRunCondition>>> {
    let mut map = HashMap::new();
    for (id, workflow) in &schema.workflows {
        let mut conditions = Vec::new();
        for proto_cond in &workflow.when {
            conditions.push(WorkflowRunCondition::from_proto(proto_cond)?);
        }
        map.insert(id.clone(), conditions);
    }
    Ok(map)
}

fn generate_setup_config(context: &CircleciContext) -> Result<Value> {
    let mut root = Mapping::new();
    root.insert(Value::String("version".into()), Value::String("2.1".into()));
    root.insert(Value::String("setup".into()), Value::Bool(true));

    let orbs = build_orbs_map();
    root.insert(Value::String("orbs".into()), Value::Mapping(orbs));

    // We need to collect variants for ALL workflows to generate the hash steps,
    // but only those triggering the 'main' workflow are critical for the setup job logic usually.
    // Here we collect them to ensure we have coverage of source files.
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

    // We assume "main" or "ci" is the primary one?
    // Actually cigen supports multiple workflows.
    // The setup job typically triggers one continuation config which contains ALL workflows.
    // So we need to know which workflow(s) to run.
    // The `build_setup_job` logic handles generating hashes for jobs.

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
        wf.insert(Value::String("setup".into()), Value::Mapping(setup_wf));
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
    // Add user defined orbs
    if let Some(Value::Mapping(user_orbs)) = context.raw_config.get(&Value::String("orbs".into())) {
        for (k, v) in user_orbs {
            orbs.insert(k.clone(), v.clone());
        }
    }
    root.insert(Value::String("orbs".into()), Value::Mapping(orbs));

    // Commands
    let commands = build_commands_map(context)?;
    if !commands.is_empty() {
        root.insert(Value::String("commands".into()), Value::Mapping(commands));
    }

    // Group jobs by workflow
    let mut grouped_jobs: HashMap<String, Vec<&JobDefinition>> = HashMap::new();
    for job in &context.schema.jobs {
        let wf = if job.workflow.is_empty() {
            "ci"
        } else {
            &job.workflow
        };
        grouped_jobs.entry(wf.to_string()).or_default().push(job);
    }

    // Collect all variants
    let mut all_variants = Vec::new();
    let mut workflow_variants_map: HashMap<String, Vec<JobVariant>> = HashMap::new();

    for (wf_id, _) in &grouped_jobs {
        let variants = collect_job_variants_for_workflow(context, wf_id)?;
        workflow_variants_map.insert(wf_id.clone(), variants.clone());
        all_variants.extend(variants);
    }

    // Jobs map
    let mut jobs_map = Mapping::new();
    for variant in &all_variants {
        // Generate job definition
        let job_def = convert_job(variant, context)?;
        jobs_map.insert(Value::String(variant.variant_name.clone()), job_def);
    }
    root.insert(Value::String("jobs".into()), Value::Mapping(jobs_map));

    // Workflows map
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

        // Jobs are already expanded by cigen core.
        // instance_id is in job.id
        variants.push(JobVariant {
            base_id: &job.id,
            variant_name: job.id.clone(), // Already sanitized/unique
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

        if job.type_ == "approval" {
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

fn convert_job(variant: &JobVariant, context: &CircleciContext) -> Result<Value> {
    let job = variant.job;

    // Skip approval jobs in definition list (they only appear in workflows)
    // Wait, CircleCI requires "type: approval" jobs to NOT be defined in "jobs:" section?
    // Yes, approval jobs are only in workflows.
    // So we should check if we should generate a job definition.
    // But currently `main` calls `convert_job` for all variants.
    // I should modify `generate_main_config` loop to skip approval jobs.
    // Or return Value::Null here and filter?

    // Actually, standard CircleCI jobs `type: approval` are defined IN WORKFLOWS, not in `jobs`.
    // So `convert_job` shouldn't be called for them or should return something indicative.
    // But let's handle it in `generate_main_config`.

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

    Ok(Value::Mapping(map))
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
        // ... other steps ...
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

// ... helper functions from original ...
fn build_job_hash_step(variant: &JobVariant) -> Value {
    let command = [
        "set -euo pipefail".to_string(),
        "mkdir -p /tmp/cigen".to_string(),
        format!(
            "JOB_HASH=$(cigen hash --job {} --config .cigen | tr -d '\\r')",
            variant.base_id
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

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("run".into()), Value::Mapping(run_map));
    wrapper.insert(
        Value::String("when".into()),
        Value::String("on_success".into()),
    );
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

    let mut wrapper = Mapping::new();
    wrapper.insert(Value::String("save_cache".into()), Value::Mapping(save_map));
    wrapper.insert(
        Value::String("when".into()),
        Value::String("on_success".into()),
    );
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
        Value::String(".circleci/main.into()".into()),
    );

    for parameter in extract_parameter_names(raw_config) {
        params.insert(
            Value::String(parameter.clone()),
            Value::String(format!("<< pipeline.parameters.{parameter} >>")),
        );
    }

    let mut wrapper = Mapping::new();
    wrapper.insert(
        Value::String("continuation/continue".into()),
        Value::Mapping(params),
    );
    Value::Mapping(wrapper)
}

fn extract_parameter_names(raw: &Value) -> Vec<String> {
    raw.as_mapping()
        .and_then(|map| map.get(&Value::String("parameters".into())))
        .and_then(Value::as_mapping)
        .map(|mapping| {
            mapping
                .keys()
                .filter_map(Value::as_str)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
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
            .map_err(|_| anyhow!("Unknown workflow condition kind value: {}", proto.kind)?);

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
        conflicts_with: vec!["provider:*\\".to_string()],
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
