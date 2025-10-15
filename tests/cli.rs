use assert_cmd::prelude::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::process::Command;
use std::time::Duration;
use tempfile::tempdir;
use trycmd::TestCases;

#[test]
fn cli_coverage() {
    let cases = TestCases::new();
    // Ensure cigen resolves to compiled bin
    cases.register_bin("cigen", std::path::Path::new(env!("CARGO_BIN_EXE_cigen")));
    // Stable env
    cases.env("CIGEN_SKIP_CIRCLECI_CLI", "1");

    // Note: paths in outputs will be absolute to the CI runner; acceptable for snapshots.

    cases.case("tests/cmd/**/*.trycmd");
}

#[test]
fn hash_subcommand_produces_deterministic_output() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("example.txt");
    std::fs::write(&file_path, "hello world")?;

    let mut cmd = Command::cargo_bin("cigen")?;
    cmd.current_dir(dir.path())
        .args(["hash", "-p", "example.txt"]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output)?;
    let produced = stdout.trim();

    let mut file_hasher = Sha256::new();
    file_hasher.update(b"hello world");
    let file_digest = file_hasher.finalize();

    let mut aggregate = Sha256::new();
    aggregate.update(b"example.txt");
    aggregate.update([0u8]);
    aggregate.update(file_digest);
    let expected = hex::encode(aggregate.finalize());

    assert_eq!(produced, expected);
    Ok(())
}

#[test]
fn hash_subcommand_persists_cache_entries() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let cache_path = dir.path().join(".cigen/cache/file-hashes.json");
    let file_path = dir.path().join("example.txt");
    fs::create_dir_all(dir.path().join(".cigen/cache"))?;
    fs::write(&file_path, "cached content")?;

    let mut cmd = Command::cargo_bin("cigen")?;
    cmd.current_dir(dir.path()).args([
        "hash",
        "-p",
        "example.txt",
        "--cache",
        cache_path.to_str().unwrap(),
    ]);
    let first_hash = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;

    let cache_contents = fs::read_to_string(&cache_path)?;
    let cache: Value = serde_json::from_str(&cache_contents)?;
    let entry = cache
        .get("example.txt")
        .ok_or("missing cache entry for example.txt")?;
    assert!(entry.get("hash").and_then(Value::as_str).is_some());
    assert!(entry.get("modified").is_some());
    assert!(entry.get("size").is_some());

    // Subsequent run with the same metadata should hit the cache and keep the same hash.
    let mut second = Command::cargo_bin("cigen")?;
    second.current_dir(dir.path()).args([
        "hash",
        "-p",
        "example.txt",
        "--cache",
        cache_path.to_str().unwrap(),
    ]);
    let output = second.assert().success().get_output().stdout.clone();
    let second_hash = String::from_utf8(output)?;
    assert_eq!(second_hash.trim(), first_hash.trim());

    let cached_hash = entry
        .get("hash")
        .and_then(Value::as_str)
        .ok_or("missing cached hash")?;
    let mut file_hasher = Sha256::new();
    file_hasher.update(b"cached content");
    assert_eq!(cached_hash, hex::encode(file_hasher.finalize()));

    Ok(())
}

#[test]
fn hash_subcommand_writes_github_output_when_requested() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("example.txt");
    fs::write(&file_path, "github output")?;
    let output_file = dir.path().join("output.txt");

    let mut cmd = Command::cargo_bin("cigen")?;
    cmd.current_dir(dir.path())
        .args(["hash", "-p", "example.txt", "--output", "job_hash"]);
    cmd.env("GITHUB_OUTPUT", &output_file);
    cmd.assert().success();

    let contents = fs::read_to_string(&output_file)?;
    assert!(contents.contains("job_hash="));
    Ok(())
}

#[test]
fn hash_job_mode_reflects_config_and_source_changes() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let repo = dir.path();

    let git = |args: &[&str]| -> Result<(), Box<dyn std::error::Error>> {
        let status = Command::new("git").args(args).current_dir(repo).status()?;
        assert!(status.success(), "git {:?} failed", args);
        Ok(())
    };

    // Initialize git repository to satisfy git ls-files lookups.
    git(&["init"])?;
    git(&["config", "user.email", "ci@example.com"])?;
    git(&["config", "user.name", "CI Agent"])?;

    // Write minimal project contents.
    fs::create_dir_all(repo.join("src"))?;
    fs::create_dir_all(repo.join(".cigen/workflows/ci/jobs"))?;

    fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;
    fs::write(repo.join("src/lib.rs"), "pub fn value() -> u8 { 1 }\n")?;

    fs::write(
        repo.join(".cigen/config.yml"),
        "provider: github-actions\nsource_file_groups:\n  rust:\n    - \"src/**/*.rs\"\n    - \"Cargo.toml\"\nworkflows:\n  ci:\n    jobs:\n      - fmt\n",
    )?;

    fs::write(
        repo.join(".cigen/workflows/ci/jobs/fmt.yml"),
        "image: rust:latest\nsource_files:\n  - \"@rust\"\nsteps:\n  - run:\n      name: Format\n      command: cargo fmt -- --check\n",
    )?;

    git(&["add", "."])?;
    let status = Command::new("git")
        .args(["commit", "-m", "initial"])
        .env("GIT_AUTHOR_NAME", "CI Agent")
        .env("GIT_AUTHOR_EMAIL", "ci@example.com")
        .env("GIT_COMMITTER_NAME", "CI Agent")
        .env("GIT_COMMITTER_EMAIL", "ci@example.com")
        .current_dir(repo)
        .status()?;
    assert!(status.success(), "initial commit failed");

    let mut cmd = Command::cargo_bin("cigen")?;
    cmd.current_dir(repo).args([
        "hash",
        "--job",
        "fmt",
        "--config",
        ".cigen",
        "--cache",
        ".cigen/cache/file-hashes.json",
    ]);
    let digest_one = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;

    let cache_contents = fs::read_to_string(repo.join(".cigen/cache/file-hashes.json"))?;
    let cache: Value = serde_json::from_str(&cache_contents)?;
    assert!(cache.get("Cargo.toml").is_some());
    assert!(cache.get("src/lib.rs").is_some());

    // Modify tracked source file; the hash should change on the next run.
    std::thread::sleep(Duration::from_secs(1));
    fs::write(repo.join("src/lib.rs"), "pub fn value() -> u8 { 2 }\n")?;

    let mut second = Command::cargo_bin("cigen")?;
    second.current_dir(repo).args([
        "hash",
        "--job",
        "fmt",
        "--config",
        ".cigen",
        "--cache",
        ".cigen/cache/file-hashes.json",
    ]);
    let digest_two = String::from_utf8(second.assert().success().get_output().stdout.clone())?;

    assert_ne!(digest_one.trim(), digest_two.trim());
    Ok(())
}
