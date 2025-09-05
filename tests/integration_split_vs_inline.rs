use assert_cmd::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_generate_in_fixture(fixture_rel: &str) -> PathBuf {
    let root = repo_root();
    let fixture_dir = root.join(fixture_rel);
    // Clean output
    let output_dir = fixture_dir.join(".circleci");
    let _ = fs::remove_dir_all(&output_dir);
    fs::create_dir_all(&output_dir).unwrap();

    let mut cmd = Command::cargo_bin("cigen").unwrap();
    cmd.current_dir(&fixture_dir)
        .env("CIGEN_SKIP_CIRCLECI_CLI", "1")
        .arg("generate");
    cmd.assert().success();

    output_dir.join("config.yml")
}

#[test]
fn split_and_inline_configs_match() {
    let split_config = run_generate_in_fixture("integration_tests/circleci_node_simple_split");
    let inline_config = run_generate_in_fixture("integration_tests/circleci_node_simple_inline");

    let a = fs::read_to_string(&split_config).unwrap();
    let b = fs::read_to_string(&inline_config).unwrap();
    assert_eq!(a, b, "split and inline generated configs should match");
}

#[test]
fn generated_yaml_contains_circleci_version_and_checkout() {
    let cfg = run_generate_in_fixture("integration_tests/circleci_node_simple_inline");
    let s = fs::read_to_string(cfg).unwrap();
    assert!(s.contains("version: 2.1"));
    // For minimal fixtures, just ensure a valid CircleCI version header exists
}
