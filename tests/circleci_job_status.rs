#![allow(clippy::needless_borrows_for_generic_args)]

use assert_cmd::prelude::*;
use cigen::loader::load_split_config;
use serde_yaml::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn jobs_with_sources(config_dir: &Path, workflow: &str) -> (HashSet<String>, HashSet<String>) {
    let config = load_split_config(config_dir).expect("failed to load config");
    let mut with_sources = HashSet::new();
    let mut without_sources = HashSet::new();

    for (id, job) in &config.jobs {
        if job.workflow.as_deref() != Some(workflow) {
            continue;
        }
        if job.source_files.is_empty() {
            without_sources.insert(id.clone());
        } else {
            with_sources.insert(id.clone());
        }
    }

    (with_sources, without_sources)
}

fn run_generate(file: &Path, output_dir: &Path) {
    let mut cmd = Command::cargo_bin("cigen").expect("cigen binary not found");
    cmd.arg("generate")
        .arg("--file")
        .arg(file)
        .arg("--output")
        .arg(output_dir)
        .current_dir(repo_root())
        .env("CIGEN_SKIP_CIRCLECI_CLI", "1");
    cmd.assert().success();
}

fn load_jobs_map(path: &Path) -> HashMap<String, Value> {
    let yaml = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    });

    let root: Value = serde_yaml::from_str(&yaml).unwrap_or_else(|err| {
        panic!("failed to parse {}: {err}", path.display());
    });

    root.as_mapping()
        .and_then(|map| map.get(&Value::String("jobs".into())))
        .and_then(Value::as_mapping)
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| key.as_str().map(|k| (k.to_string(), value.clone())))
                .collect()
        })
        .unwrap_or_default()
}

fn assert_has_job_status_steps(job_id: &str, job_value: &Value) {
    let steps = extract_steps(job_id, job_value);

    let has_hash_step = steps
        .iter()
        .any(|step| is_named_run_step(step, "Compute job hash"));
    assert!(
        has_hash_step,
        "job {job_id} is missing 'Compute job hash' step"
    );

    let mut has_persist_step = false;
    for step in steps {
        if let Some(save_cache_map) = step
            .as_mapping()
            .and_then(|map| map.get(&Value::String("save_cache".into())))
            .and_then(Value::as_mapping)
        {
            let name_matches = save_cache_map
                .get(&Value::String("name".into()))
                .and_then(Value::as_str)
                == Some("Persist job status");
            let key_matches = save_cache_map
                .get(&Value::String("key".into()))
                .and_then(Value::as_str)
                .map(|value| value.contains("job_status-exists-v1"))
                .unwrap_or(false);
            let when_matches = step
                .as_mapping()
                .and_then(|map| map.get(&Value::String("when".into())))
                .and_then(Value::as_str)
                == Some("on_success");
            if name_matches && key_matches && when_matches {
                has_persist_step = true;
                break;
            }
        }
    }

    assert!(
        has_persist_step,
        "job {job_id} is missing 'Persist job status' save_cache step"
    );
}

fn assert_absent_job_status_steps(job_id: &str, job_value: &Value) {
    let steps = extract_steps(job_id, job_value);

    let has_hash_step = steps
        .iter()
        .any(|step| is_named_run_step(step, "Compute job hash"));
    assert!(
        !has_hash_step,
        "job {job_id} should not include 'Compute job hash' step"
    );

    let has_persist_step = steps.iter().any(|step| {
        step.as_mapping()
            .and_then(|map| map.get(&Value::String("save_cache".into())))
            .and_then(Value::as_mapping)
            .and_then(|save| {
                save.get(&Value::String("name".into()))
                    .and_then(Value::as_str)
            })
            == Some("Persist job status")
    });
    assert!(
        !has_persist_step,
        "job {job_id} should not include 'Persist job status' save_cache step"
    );
}

fn extract_steps<'a>(job_id: &str, job_value: &'a Value) -> &'a Vec<Value> {
    job_value
        .as_mapping()
        .and_then(|map| map.get(&Value::String("steps".into())))
        .and_then(Value::as_sequence)
        .unwrap_or_else(|| {
            panic!("job {job_id} has no steps array");
        })
}

fn is_named_run_step(step: &Value, expected_name: &str) -> bool {
    step.as_mapping()
        .and_then(|map| map.get(&Value::String("run".into())))
        .and_then(Value::as_mapping)
        .and_then(|run_map| run_map.get(&Value::String("name".into())))
        .and_then(Value::as_str)
        == Some(expected_name)
}

#[test]
fn circleci_job_status_steps_for_rails_fixture() {
    let root = repo_root();
    let config_dir = root.join("integration_tests/circleci_rails/.cigen");
    let (with_sources, without_sources) = jobs_with_sources(&config_dir, "main");

    let output = tempdir().expect("failed to create tempdir");
    run_generate(&config_dir, output.path());

    let jobs = load_jobs_map(&output.path().join(".circleci/main.yml"));

    for job_id in &with_sources {
        let job_value = jobs.get(job_id).unwrap_or_else(|| {
            panic!("expected job {job_id} in generated main.yml");
        });
        assert_has_job_status_steps(job_id, job_value);
    }

    for job_id in &without_sources {
        if let Some(job_value) = jobs.get(job_id) {
            assert_absent_job_status_steps(job_id, job_value);
        }
    }
}

#[test]
fn circleci_job_status_steps_for_docspring_config() {
    let root = repo_root();
    let config_dir = root.join("docspring/.cigen");
    if !config_dir.exists() {
        eprintln!("docspring/.cigen not found – skipping docspring job status test");
        return;
    }

    let (with_sources, without_sources) = jobs_with_sources(&config_dir, "main");

    let output = tempdir().expect("failed to create tempdir");
    run_generate(&config_dir, output.path());

    let jobs = load_jobs_map(&output.path().join(".circleci/main.yml"));

    for job_id in &with_sources {
        let job_value = jobs.get(job_id).unwrap_or_else(|| {
            panic!("expected job {job_id} in generated main.yml");
        });
        assert_has_job_status_steps(job_id, job_value);
    }

    for job_id in &without_sources {
        if let Some(job_value) = jobs.get(job_id) {
            assert_absent_job_status_steps(job_id, job_value);
        }
    }
}
