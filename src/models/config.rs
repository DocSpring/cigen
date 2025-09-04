use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_filename: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchors: Option<HashMap<String, serde_json::Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub caches: Option<CacheBackendConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_definitions: Option<HashMap<String, CacheDefinition>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_sources: Option<HashMap<String, Vec<VersionSource>>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub architectures: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_classes: Option<HashMap<String, HashMap<String, String>>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker: Option<DockerConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<HashMap<String, Service>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file_groups: Option<HashMap<String, Vec<String>>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars: Option<HashMap<String, serde_yaml::Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph: Option<GraphConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, ParameterConfig>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub orbs: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<OutputConfig>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker_images: Option<HashMap<String, DockerImageConfig>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_managers:
        Option<HashMap<String, super::package_managers::PackageManagerDefinition>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub template: String,
    pub output: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterConfig {
    /// Parameter type (boolean, string, integer, etc.)
    #[serde(rename = "type")]
    pub param_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerImageConfig {
    /// Default image used when no architecture-specific image is available
    pub default: String,

    /// Architecture-specific image variants
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architectures: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_auth: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<HashMap<String, DockerAuth>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub image: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<ServiceEnvironment>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServiceEnvironment {
    /// Environment variables as key-value pairs
    Map(HashMap<String, String>),
    /// Environment variables as array of KEY=value strings
    Array(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheBackendConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<CacheBackend>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_status: Option<CacheBackend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheBackend {
    pub backend: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cache {
    pub key: String,
    pub paths: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_always: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: "circleci".to_string(),
            output_path: None,
            output_filename: None,
            version: None,
            anchors: None,
            caches: None,
            cache_definitions: None,
            version_sources: None,
            architectures: None,
            resource_classes: None,
            docker: None,
            services: None,
            source_file_groups: None,
            vars: None,
            graph: None,
            dynamic: None,
            setup: None,
            parameters: None,
            orbs: None,
            outputs: None,
            docker_images: None,
            package_managers: None,
        }
    }
}

impl Config {
    /// Load config from YAML content, merging split configs if provided
    pub fn from_yaml(content: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(content)
    }

    /// Get all service names defined in this config
    pub fn service_names(&self) -> Vec<&String> {
        self.services
            .as_ref()
            .map(|services| services.keys().collect())
            .unwrap_or_default()
    }

    /// Get cache backend names defined in this config
    pub fn cache_backend_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        if let Some(cache_backend_config) = &self.caches {
            if cache_backend_config.artifacts.is_some() {
                names.push("artifacts");
            }
            if cache_backend_config.job_status.is_some() {
                names.push("job_status");
            }
        }
        names
    }

    /// Get all source file group names defined in this config
    pub fn source_file_group_names(&self) -> Vec<&String> {
        self.source_file_groups
            .as_ref()
            .map(|groups| groups.keys().collect())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheDefinition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<PathOrDetect>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum_sources: Option<Vec<PathOrDetect>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<PathOrDetect>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PathOrDetect {
    Path(String),
    Detect { detect: Vec<String> },
    DetectOptional { detect_optional: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VersionSource {
    File {
        file: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
    },
    Command {
        command: String,
        #[serde(default = "default_parse_version")]
        parse_version: bool,
    },
}

fn default_parse_version() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultCacheConfig {
    pub cache_definitions: HashMap<String, CacheDefinition>,
    pub version_sources: HashMap<String, Vec<VersionSource>>,
}
