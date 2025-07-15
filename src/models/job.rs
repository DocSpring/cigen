use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub image: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub architectures: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_class: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_files: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallelism: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_cache: Option<Vec<CacheRestore>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<Step>>,
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
#[serde(untagged)]
pub enum Step {
    Command(String),
    Complex {
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        run: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        store_test_results: Option<StoreTestResults>,

        #[serde(skip_serializing_if = "Option::is_none")]
        store_artifacts: Option<StoreArtifacts>,
    },
}

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
}
