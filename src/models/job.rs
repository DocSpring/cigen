use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Represents job dependencies that can be either a single string or array of strings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JobRequires {
    Single(String),
    Multiple(Vec<String>),
}

impl JobRequires {
    /// Convert to a normalized vector of job names
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            JobRequires::Single(s) => vec![s.clone()],
            JobRequires::Multiple(v) => v.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub image: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub architectures: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_class: Option<String>,

    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub source_files: Option<Vec<String>>,

    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub source_submodules: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallelism: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<JobRequires>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "deserialize_cache_definitions")]
    pub cache: Option<HashMap<String, CacheDefinition>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_cache: Option<Vec<CacheRestore>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<String>>,

    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        deserialize_with = "deserialize_packages"
    )]
    pub packages: Option<Vec<PackageSpec>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<Step>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<super::config::CheckoutSetting>,

    /// CircleCI job type (e.g., approval)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub job_type: Option<String>,
}

/// Intermediate parsing structure for cache definitions that handles multiple YAML formats
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CacheDefinitionRaw {
    /// Simple string format: `cache_name: "path/to/cache"`
    Simple(String),
    /// Array format: `cache_name: ["path1", "path2"]`
    Array(Vec<String>),
    /// Object format: `cache_name: { paths: ["path1"], restore: false }`
    Object {
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<CachePathSpec>,
        #[serde(skip_serializing_if = "Option::is_none")]
        paths: Option<CachePathSpec>,
        #[serde(skip_serializing_if = "Option::is_none")]
        restore: Option<bool>,
    },
}

/// Path specification that can be either a string or array
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CachePathSpec {
    Single(String),
    Multiple(Vec<String>),
}

/// Final normalized cache definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheDefinition {
    pub paths: Vec<String>,
    pub restore: bool,
}

impl From<CacheDefinitionRaw> for CacheDefinition {
    fn from(raw: CacheDefinitionRaw) -> Self {
        match raw {
            CacheDefinitionRaw::Simple(path) => CacheDefinition {
                paths: vec![path],
                restore: true,
            },
            CacheDefinitionRaw::Array(paths) => CacheDefinition {
                paths,
                restore: true,
            },
            CacheDefinitionRaw::Object {
                path,
                paths,
                restore,
            } => {
                let final_paths = match (path, paths) {
                    (Some(path_spec), None) => match path_spec {
                        CachePathSpec::Single(p) => vec![p],
                        CachePathSpec::Multiple(ps) => ps,
                    },
                    (None, Some(paths_spec)) => match paths_spec {
                        CachePathSpec::Single(p) => vec![p],
                        CachePathSpec::Multiple(ps) => ps,
                    },
                    (Some(_), Some(paths_spec)) => {
                        // If both are specified, prefer paths
                        match paths_spec {
                            CachePathSpec::Single(p) => vec![p],
                            CachePathSpec::Multiple(ps) => ps,
                        }
                    }
                    (None, None) => vec![], // Invalid but we'll let validation catch this
                };

                CacheDefinition {
                    paths: final_paths,
                    restore: restore.unwrap_or(true),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CacheRestore {
    Simple(String),
    Complex {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        dependency: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Step(pub serde_yaml::Value);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreTestResults {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreArtifacts {
    pub path: String,
}

impl Job {
    /// Load job from YAML content
    pub fn from_yaml(content: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(content)
    }

    /// Get all service references in this job
    pub fn service_references(&self) -> Vec<&String> {
        self.services
            .as_ref()
            .map(|s| s.iter().collect())
            .unwrap_or_default()
    }

    /// Get all cache references in this job
    pub fn cache_references(&self) -> Vec<&str> {
        let mut refs = Vec::new();

        if let Some(restore_cache) = &self.restore_cache {
            for cache in restore_cache {
                match cache {
                    CacheRestore::Simple(name) => refs.push(name.as_str()),
                    CacheRestore::Complex { name, .. } => refs.push(name.as_str()),
                }
            }
        }

        refs
    }

    /// Get explicit job dependencies
    pub fn required_jobs(&self) -> Vec<String> {
        self.requires
            .as_ref()
            .map(|r| r.to_vec())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum PackageSpec {
    /// Simple shorthand: "node", "ruby", etc.
    Simple(String),
    /// Extended form with path scoping (e.g., install under a subdirectory)
    WithPath { name: String, path: String },
}

impl PackageSpec {
    pub fn name(&self) -> &str {
        match self {
            PackageSpec::Simple(s) => s.as_str(),
            PackageSpec::WithPath { name, .. } => name.as_str(),
        }
    }

    pub fn path(&self) -> Option<&str> {
        match self {
            PackageSpec::Simple(_) => None,
            PackageSpec::WithPath { path, .. } => Some(path.as_str()),
        }
    }
}

fn deserialize_packages<'de, D>(deserializer: D) -> Result<Option<Vec<PackageSpec>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let value: Option<serde_yaml::Value> = Option::deserialize(deserializer)?;

    match value {
        None => Ok(None),
        Some(serde_yaml::Value::String(s)) => Ok(Some(vec![PackageSpec::Simple(s)])),
        Some(serde_yaml::Value::Sequence(seq)) => {
            let mut out: Vec<PackageSpec> = Vec::new();
            for item in seq {
                match item {
                    serde_yaml::Value::String(s) => out.push(PackageSpec::Simple(s)),
                    serde_yaml::Value::Mapping(_) => {
                        let spec: PackageSpec =
                            serde_yaml::from_value(item).map_err(D::Error::custom)?;
                        out.push(spec);
                    }
                    _ => return Err(D::Error::custom("Invalid packages entry")),
                }
            }
            Ok(Some(out))
        }
        Some(serde_yaml::Value::Mapping(map)) => {
            // Support single object form: packages: { name: node, path: docs }
            let spec: PackageSpec = serde_yaml::from_value(serde_yaml::Value::Mapping(map))
                .map_err(D::Error::custom)?;
            Ok(Some(vec![spec]))
        }
        Some(_) => Err(D::Error::custom(
            "packages must be a string, array, or object",
        )),
    }
}

/// Custom deserializer for cache definitions that handles multiple YAML formats
fn deserialize_cache_definitions<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, CacheDefinition>>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw_map: Option<HashMap<String, CacheDefinitionRaw>> = Option::deserialize(deserializer)?;

    match raw_map {
        Some(map) => {
            let converted_map = map.into_iter().map(|(k, v)| (k, v.into())).collect();
            Ok(Some(converted_map))
        }
        None => Ok(None),
    }
}

fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let value: Option<serde_yaml::Value> = Option::deserialize(deserializer)?;

    match value {
        None => Ok(None),
        Some(serde_yaml::Value::String(s)) => Ok(Some(vec![s])),
        Some(serde_yaml::Value::Sequence(seq)) => {
            let mut result = Vec::new();
            for item in seq {
                match item {
                    serde_yaml::Value::String(s) => result.push(s),
                    _ => {
                        return Err(D::Error::custom("Array must contain only strings"));
                    }
                }
            }
            Ok(Some(result))
        }
        Some(_) => Err(D::Error::custom(
            "Field must be a string or array of strings",
        )),
    }
}
