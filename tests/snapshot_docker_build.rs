use assert_cmd::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
#[ignore = "CircleCI plugin not implemented yet"]
fn snapshot_docker_build_minimal() {
    let root = repo_root();
    let fixture_dir = root.join("integration_tests/circleci_docker_build_minimal");
    let output_dir = fixture_dir.join(".circleci");
    let _ = fs::remove_dir_all(&output_dir);
    fs::create_dir_all(&output_dir).unwrap();

    let mut cmd = Command::cargo_bin("cigen").unwrap();
    cmd.current_dir(&fixture_dir)
        .env("CIGEN_SKIP_CIRCLECI_CLI", "1")
        .arg("generate");
    cmd.assert().success();

    let yaml = fs::read_to_string(output_dir.join("config.yml")).unwrap();

    // Should contain build job and resolved image tag
    assert!(yaml.contains("build_ci_base"), "missing build job");
    assert!(
        yaml.contains("example/repo/ci_base:"),
        "missing resolved image tag"
    );
    assert!(yaml.contains("requires:"), "missing requires between jobs");
}

#[test]
#[ignore = "CircleCI plugin not implemented yet"]
fn snapshot_docker_build_with_layer_cache() {
    let root = repo_root();
    let fixture_dir = root.join("integration_tests/circleci_docker_build_layer_cache");
    let output_dir = fixture_dir.join(".circleci");
    let _ = fs::remove_dir_all(&output_dir);
    fs::create_dir_all(&output_dir).unwrap();

    let mut cmd = Command::cargo_bin("cigen").unwrap();
    cmd.current_dir(&fixture_dir)
        .env("CIGEN_SKIP_CIRCLECI_CLI", "1")
        .arg("generate");
    cmd.assert().success();

    let yaml = fs::read_to_string(output_dir.join("config.yml")).unwrap();
    assert!(
        yaml.contains("setup_remote_docker:"),
        "missing setup_remote_docker step"
    );
    assert!(
        yaml.contains("docker_layer_caching: true"),
        "missing docker_layer_caching flag"
    );
}
