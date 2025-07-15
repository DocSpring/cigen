use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;
use tracing::debug;

pub struct ConfigMerger;

impl ConfigMerger {
    pub fn new() -> Self {
        Self
    }

    pub fn merge_configs(
        &self,
        mut base: Value,
        fragments: Vec<(PathBuf, Value)>,
    ) -> Result<Value> {
        for (path, fragment) in fragments {
            debug!("  Merging {:?}...", path.file_name().unwrap());
            Self::deep_merge(&mut base, fragment)?;
        }
        Ok(base)
    }

    fn deep_merge(base: &mut Value, other: Value) -> Result<()> {
        match (base, other) {
            (Value::Object(base_map), Value::Object(other_map)) => {
                for (key, value) in other_map {
                    match base_map.get_mut(&key) {
                        Some(base_value) => {
                            // Recursively merge nested objects
                            Self::deep_merge(base_value, value)?;
                        }
                        None => {
                            // Add new key
                            base_map.insert(key, value);
                        }
                    }
                }
            }
            (base_val, other_val) => {
                // For non-objects, the fragment value overwrites the base
                *base_val = other_val;
            }
        }
        Ok(())
    }
}
