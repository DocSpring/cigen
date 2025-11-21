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
    base_id: &'a str,
    variant_name: String,
    job: &'a JobDefinition,
    matrix_values: HashMap<String, String>,
    stage: String,
}

// ...

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

        // 1. Handle architectures (legacy/specific behavior)
        let arches = parse_architectures(job)?;
        
        // 2. Handle standard matrix
        let matrix_combinations = expand_job_matrix(job);

        if arches.is_empty() && matrix_combinations.is_empty() {
            variants.push(JobVariant {
                base_id: &job.id,
                variant_name: sanitize_job_name(&job.id),
                job,
                matrix_values: HashMap::new(),
                stage: if job.stage.is_empty() { "default".to_string() } else { job.stage.clone() },
            });
        } else {
            // Combine architectures and matrix
            // If architectures are present, they act like another matrix dimension "arch"
            
            let mut dimensions = Vec::new();
            if !arches.is_empty() {
                dimensions.push(("arch".to_string(), arches));
            }
            
            // Flatten matrix combinations back to dimensions for cartesian product if needed,
            // or just treat the matrix combinations as one set of variants.
            // For simplicity, let's assume standard matrix is used OR architectures, not both mixed deeply yet,
            // or we strictly cross product them.
            
            // Let's iterate arches (if any) AND matrix combos.
            
            let arch_list = if arches.is_empty() { vec![String::new()] } else { arches };
            let combo_list = if matrix_combinations.is_empty() { vec![HashMap::new()] } else { matrix_combinations };

            for arch in &arch_list {
                for combo in &combo_list {
                    let mut final_matrix = combo.clone();
                    // Sanitize job ID part of name
                    let mut name_parts = vec![sanitize_job_name(&job.id)];
                    
                    // Append matrix values to name, sorted by key
                    let mut sorted_keys: Vec<_> = combo.keys().collect();
                    sorted_keys.sort();
                    for key in sorted_keys {
                        if let Some(v) = combo.get(key) {
                            // Don't include stage in name parts if it's "stage" key?
                            // Usually we include all matrix vars.
                            // But if stage is used for grouping, maybe we don't want it in the suffix?
                            // The user's example had `deploy_us_pre_release`.
                            // If `stage` is `deploy_us` (from matrix), and we include it in name parts...
                            // name parts: `pre_release`, `deploy_us`.
                            // joined: `pre_release_deploy_us`.
                            // Then prefix: `deploy_us_pre_release_deploy_us`.
                            // This is redundant.
                            // So we should EXCLUDE "stage" from name parts if we use it as prefix.
                            if key != "stage" {
                                name_parts.push(v.clone());
                            }
                        }
                    }

                    if !arch.is_empty() {
                        final_matrix.insert("arch".to_string(), arch.clone());
                        name_parts.push(arch.clone());
                    }

                    // Determine stage
                    let stage = final_matrix.get("stage").cloned()
                        .or_else(|| if job.stage.is_empty() { None } else { Some(job.stage.clone()) })
                        .unwrap_or_else(|| "default".to_string());

                    variants.push(JobVariant {
                        base_id: &job.id,
                        variant_name: name_parts.join("_"),
                        job,
                        matrix_values: final_matrix,
                        stage,
                    });
                }
            }
        }
    }
    Ok(variants)
}

fn expand_job_matrix(job: &JobDefinition) -> Vec<HashMap<String, String>> {
    // Priority 1: Explicit rows
    if !job.matrix_rows.is_empty() {
        return job.matrix_rows.iter().map(|r| r.values.clone()).collect();
    }

    // Priority 2: Dimensions for cartesian product
    // Only proceed if dimensions actually exist
    if !job.matrix.is_empty() { 
        let mut dimensions: Vec<(String, Vec<String>)> = Vec::new();
        for (key, value) in &job.matrix {
            dimensions.push((key.clone(), value.values.clone()));
        }
        dimensions.sort_by(|a, b| a.0.cmp(&b.0));

        let combinations = cartesian_product(&dimensions);
        
        return combinations.into_iter().map(|combo| {
            combo.into_iter().collect()
        }).collect();
    }

    // No matrix defined
    Vec::new()
}

fn cartesian_product(dimensions: &[(String, Vec<String>)]) -> Vec<Vec<(String, String)>> {
    if dimensions.is_empty() {
        return vec![vec![]];
    }

    let (key, values) = &dimensions[0];
    let rest = cartesian_product(&dimensions[1..]);

    let mut result = Vec::new();
    for value in values {
        for combo in &rest {
            let mut new_combo = vec![(key.clone(), value.clone())];
            new_combo.extend(combo.clone());
            result.push(new_combo);
        }
    }

    result
}


fn parse_architectures(job: &JobDefinition) -> Result<Vec<String>> {
    if let Some(raw) = job.extra.get("architectures") {
        let value: Value = serde_yaml::from_str(raw)
            .with_context(|| format!("Failed to parse architectures for job {}", job.id))?;
        if let Value::Sequence(items) = value {
            let mut arches = Vec::new();
            for item in items {
                if let Some(arch) = item.as_str() {
                    arches.push(arch.to_string());
                }
            }
            return Ok(arches);
        }
    }
    Ok(Vec::new())
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
        Value::String(".circleci/main.yml".into()),
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

fn convert_job(variant: &JobVariant, context: &CircleciContext) -> Result<Value> {
    let job = variant.job;
    let mut map = Mapping::new();

    let mut docker_entries = Vec::new();
    if !job.image.is_empty() {
        let mut image_map = Mapping::new();
        image_map.insert(
            Value::String("image".into()),
            Value::String(substitute_matrix(&job.image, &variant.matrix_values)),
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
    // Inject matrix values as environment variables
    for (key, value) in &variant.matrix_values {
        env_map.insert(
            Value::String(key.clone()),
            Value::String(value.clone()),
        );
    }
    
    if !job.env.is_empty() {
        for (key, value) in &job.env {
            env_map.insert(
                Value::String(key.clone()), 
                Value::String(substitute_matrix(value, &variant.matrix_values))
            );
        }
    }
    
    if !env_map.is_empty() {
        map.insert(Value::String("environment".into()), Value::Mapping(env_map));
    }

    if !job.runner.is_empty() {
        map.insert(
            Value::String("executor".into()),
            Value::String(substitute_matrix(&job.runner, &variant.matrix_values)),
        );
    }

    if let Some(resource_class_value) = job.extra.get("resource_class") {
        // Note: extra values are JSON serialized strings in the proto
        let mut val = parse_yaml_value(resource_class_value)?;
        substitute_matrix_in_value(&mut val, &variant.matrix_values);
        map.insert(
            Value::String("resource_class".into()),
            val,
        );
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
    steps.extend(convert_steps_list(&job.steps, &variant.matrix_values)?);
    if !job.source_files.is_empty() {
        steps.push(build_job_completion_marker_step(job));
        steps.push(build_job_status_save_step(job));
    }
    map.insert(Value::String("steps".into()), Value::Sequence(steps));

    Ok(Value::Mapping(map))
}

fn substitute_matrix(input: &str, matrix: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, value) in matrix {
        // Support ${{ matrix.key }}
        let pattern = format!("${{{{ matrix.{} }}}}", key);
        result = result.replace(&pattern, value);
        // Also support ${{ key }} for convenience if unambiguous? 
        // Stick to matrix namespace for now to avoid collisions.
    }
    result
}

fn sanitize_job_name(name: &str) -> String {
    name.replace(['/', '\\'], "_")
}

fn substitute_matrix_in_value(value: &mut Value, matrix: &HashMap<String, String>) {
    match value {
        Value::String(s) => *s = substitute_matrix(s, matrix),
        Value::Sequence(seq) => {
            for item in seq {
                substitute_matrix_in_value(item, matrix);
            }
        }
        Value::Mapping(map) => {
            for (_, v) in map.iter_mut() {
                substitute_matrix_in_value(v, matrix);
            }
        }
        _ => {}
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

fn convert_steps_list(steps: &[Step], matrix: &HashMap<String, String>) -> Result<Vec<Value>> {
    let mut converted = Vec::new();
    for step in steps {
        converted.push(convert_step(step, matrix)?);
    }
    Ok(converted)
}

fn convert_step(step: &Step, matrix: &HashMap<String, String>) -> Result<Value> {
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
                run_map.insert(Value::String("name".into()), Value::String(substitute_matrix(name, matrix)));
            }
            run_map.insert(
                Value::String("command".into()),
                Value::String(substitute_matrix(command, matrix)),
            );
            if !env.is_empty() {
                let mut env_map = Mapping::new();
                for (key, value) in env {
                    env_map.insert(
                        Value::String(key.clone()), 
                        Value::String(substitute_matrix(value, matrix))
                    );
                }
                run_map.insert(Value::String("environment".into()), Value::Mapping(env_map));
            }
            if !r#if.is_empty() {
                run_map.insert(Value::String("if".into()), Value::String(substitute_matrix(r#if, matrix)));
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
                    let mut val = parse_yaml_value(value)?;
                    substitute_matrix_in_value(&mut val, matrix);
                    with_map.insert(Value::String(key.clone()), val);
                }
                uses_map.insert(Value::String("with".into()), Value::Mapping(with_map));
            }
            if !r#if.is_empty() {
                uses_map.insert(Value::String("if".into()), Value::String(substitute_matrix(r#if, matrix)));
            }
            Ok(Value::Mapping(uses_map))
        }
        cigen::plugin::protocol::step::StepType::RestoreCache(step) => {
            let mut restore_map = Mapping::new();
            if !step.name.is_empty() {
                restore_map.insert(
                    Value::String("name".into()),
                    Value::String(substitute_matrix(&step.name, matrix)),
                );
            }
            restore_map.insert(Value::String("key".into()), Value::String(substitute_matrix(&step.key, matrix)));
            if !step.keys.is_empty() {
                restore_map.insert(
                    Value::String("keys".into()),
                    Value::Sequence(step.keys.iter().map(|k| Value::String(substitute_matrix(k, matrix))).collect()),
                );
            }
            if !step.restore_keys.is_empty() {
                restore_map.insert(
                    Value::String("restore_keys".into()),
                    Value::Sequence(
                        step.restore_keys
                            .iter()
                            .map(|k| Value::String(substitute_matrix(k, matrix)))
                            .collect(),
                    ),
                );
            }
            if !step.extra.is_empty() {
                for (key, value) in &step.extra {
                    let mut val = parse_yaml_value(value)?;
                    substitute_matrix_in_value(&mut val, matrix);
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
                    Value::String(substitute_matrix(&step.name, matrix)),
                );
            }
            save_map.insert(Value::String("key".into()), Value::String(substitute_matrix(&step.key, matrix)));
            if !step.paths.is_empty() {
                save_map.insert(
                    Value::String("paths".into()),
                    Value::Sequence(
                        step.paths
                            .iter()
                            .map(|p| Value::String(substitute_matrix(p, matrix)))
                            .collect(),
                    ),
                );
            }
            if !step.extra.is_empty() {
                for (key, value) in &step.extra {
                    let mut val = parse_yaml_value(value)?;
                    substitute_matrix_in_value(&mut val, matrix);
                    save_map.insert(Value::String(key.clone()), val);
                }
            }
            let mut wrapper = Mapping::new();
            wrapper.insert(Value::String("save_cache".into()), Value::Mapping(save_map));
            Ok(Value::Mapping(wrapper))
        }
        cigen::plugin::protocol::step::StepType::Custom(CustomStep { yaml, .. }) => {
            let mut val = parse_yaml_value(yaml)?;
            substitute_matrix_in_value(&mut val, matrix);
            Ok(val)
        }
    }
}

#[allow(clippy::collapsible_if)]
fn build_setup_workflows_map(
    context: &CircleciContext,
    grouped_jobs: &HashMap<String, Vec<&JobDefinition>>,
    main_workflow_id: &str,
    variant_map: &HashMap<String, Vec<String>>,
) -> Result<Mapping> {
    let mut workflows = Mapping::new();

    let mut setup_map = Mapping::new();
    setup_map.insert(
        Value::String("jobs".into()),
        Value::Sequence(vec![Value::String("setup".into())]),
    );
    workflows.insert(Value::String("setup".into()), Value::Mapping(setup_map));

    for (workflow_id, _) in grouped_jobs {
        if workflow_id == main_workflow_id {
            continue;
        }
        
        let variants = collect_job_variants_for_workflow(context, workflow_id)?;

        let mut workflow_map = Mapping::new();
        if let Some(conditions) = context.workflow_conditions.get(workflow_id) {
            if let Some(when_value) = build_circleci_when(conditions)? {
                workflow_map.insert(Value::String("when".into()), when_value);
            }
        }
        workflow_map.insert(
            Value::String("jobs".into()),
            Value::Sequence(build_workflow_jobs_sequence(&variants, variant_map)),
        );
        workflows.insert(
            Value::String(workflow_id.clone()),
            Value::Mapping(workflow_map),
        );
    }

    Ok(workflows)
}


#[allow(clippy::collapsible_if)]
fn build_main_workflows_map(
    context: &CircleciContext,
    main_workflow_id: &str,
    main_variants: &[JobVariant], // Changed to main_variants
    variant_map: &HashMap<String, Vec<String>>,
) -> Result<Mapping> {
    let mut workflow_map = Mapping::new();
    workflow_map.insert(
        Value::String("jobs".into()),
        Value::Sequence(build_workflow_jobs_sequence(main_variants, variant_map)), // Pass variants
    );

    if let Some(conditions) = context.workflow_conditions.get(main_workflow_id) {
        if let Some(when_value) = build_circleci_when(conditions)? {
            workflow_map.insert(Value::String("when".into()), when_value);
        }
    }

    let mut workflows = Mapping::new();
    workflows.insert(
        Value::String(main_workflow_id.to_string()),
        Value::Mapping(workflow_map),
    );
    Ok(workflows)
}

fn build_workflow_jobs_sequence(variants: &[JobVariant], variant_map: &HashMap<String, Vec<String>>) -> Vec<Value> {
    let mut entries = Vec::new();
    for variant in variants {
        let job = variant.job;
        if job.needs.is_empty() {
            entries.push(Value::String(variant.variant_name.clone()));
        } else {
            let mut requires = Vec::new();
            for need in &job.needs {
                let substituted_need = substitute_matrix(need, &variant.matrix_values);
                if let Some(deps) = variant_map.get(&substituted_need) {
                    for dep in deps {
                        requires.push(Value::String(dep.clone()));
                    }
                } else {
                    // Fallback if not found (shouldn't happen if validation passes, or if referring to specific variant)
                    requires.push(Value::String(substituted_need));
                }
            }
            let mut job_config = Mapping::new();
            job_config.insert(Value::String("requires".into()), Value::Sequence(requires));
            let mut wrapper = Mapping::new();
            wrapper.insert(Value::String(variant.variant_name.clone()), Value::Mapping(job_config));
            entries.push(Value::Mapping(wrapper));
        }
    }
    entries
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

fn extract_services(raw_config: &Value) -> HashMap<String, ServiceDefinition> {
    let mut services = HashMap::new();

    let Value::Mapping(root) = raw_config else {
        return services;
    };

    let Some(Value::Mapping(service_map)) = root.get(&Value::String("services".into())) else {
        return services;
    };

    for (key, value) in service_map {
        let Some(name) = key.as_str() else {
            continue;
        };
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
        bail!("setup_options must be a mapping");
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
