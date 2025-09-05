use assert_cmd::prelude::*;
use insta::assert_snapshot;
use serde_yaml::{Mapping, Value};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn snapshot_node_simple_split_config() {
    let root = repo_root();
    let fixture = root.join("integration_tests/circleci_node_simple_split");
    let output_dir = fixture.join(".circleci");
    let _ = fs::remove_dir_all(&output_dir);
    fs::create_dir_all(&output_dir).unwrap();

    let mut cmd = Command::cargo_bin("cigen").unwrap();
    cmd.current_dir(&fixture)
        .env("CIGEN_SKIP_CIRCLECI_CLI", "1")
        .arg("generate");
    cmd.assert().success();

    let content = fs::read_to_string(output_dir.join("config.yml")).unwrap();
    let normalized = normalize_yaml(&content);
    assert_snapshot!("node_simple_split_config", normalized);
    fn normalize_yaml(yaml: &str) -> String {
        fn sort_value(v: &Value) -> Value {
            match v {
                Value::Mapping(map) => {
                    let mut entries: Vec<(String, Value)> = map
                        .iter()
                        .map(|(k, v)| (k.as_str().unwrap_or("").to_string(), sort_value(v)))
                        .collect();
                    entries.sort_by(|a, b| a.0.cmp(&b.0));
                    let mut new_map = Mapping::new();
                    for (k, v) in entries {
                        new_map.insert(Value::String(k), v);
                    }
                    Value::Mapping(new_map)
                }
                Value::Sequence(seq) => Value::Sequence(seq.iter().map(sort_value).collect()),
                _ => v.clone(),
            }
        }

        let v: Value = serde_yaml::from_str(yaml).unwrap();
        let sorted = sort_value(&v);
        serde_yaml::to_string(&sorted).unwrap()
    }
}
