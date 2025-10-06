use assert_cmd::prelude::*;
use serde_yaml::Value as Yaml;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn generate(fixture_rel: &str) -> String {
    let root = repo_root();
    let fixture = root.join(fixture_rel);
    let out = fixture.join(".circleci");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).unwrap();
    let mut cmd = Command::cargo_bin("cigen").unwrap();
    cmd.current_dir(&fixture)
        .env("CIGEN_SKIP_CIRCLECI_CLI", "1")
        .arg("generate");
    cmd.assert().success();
    fs::read_to_string(out.join("config.yml")).unwrap()
}

#[test]
#[ignore = "CircleCI plugin not implemented yet"]
fn structure_circleci_minimal() {
    let s = generate("integration_tests/circleci_node_simple_split");
    // Basic required sections
    assert!(s.contains("version: 2.1"));
    assert!(s.contains("workflows:"));
}

#[test]
#[ignore = "CircleCI plugin not implemented yet"]
fn structure_circleci_rails_has_jobs() {
    // Rails fixture writes to ./build
    let root = repo_root();
    let fixture = root.join("integration_tests/circleci_rails");
    let out_dir = fixture.join("build");
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).unwrap();
    let mut cmd = Command::cargo_bin("cigen").unwrap();
    cmd.current_dir(&fixture)
        .env("CIGEN_SKIP_CIRCLECI_CLI", "1")
        .arg("generate");
    cmd.assert().success();
    let s = fs::read_to_string(out_dir.join("config.yml")).unwrap();
    let yaml: Yaml = serde_yaml::from_str(&s).unwrap();
    assert!(yaml.get("jobs").is_some());
    assert!(yaml.get("workflows").is_some());
}
