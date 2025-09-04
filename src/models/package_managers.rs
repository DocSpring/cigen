use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Package manager detection and installation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManagerDefinition {
    /// Versions to include in cache keys (e.g., ["node", "npm"])
    #[serde(default)]
    pub versions: Vec<String>,

    /// Detection rules for specific package managers within this family
    pub detect: Vec<PackageManagerDetection>,

    /// Files to include in cache checksum
    pub checksum_sources: Vec<ChecksumSource>,

    /// Paths to cache
    pub cache_paths: Vec<String>,
}

/// Detection rule for a specific package manager tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManagerDetection {
    /// Name of the tool (e.g., "npm", "yarn", "pip")
    pub name: String,

    /// Lock file that indicates this tool is used
    pub lockfile: String,

    /// Command to run for installation
    pub command: String,

    /// Additional conditions (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

/// Checksum source specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChecksumSource {
    /// Single file
    File(String),
    /// Detect one of these files
    Detect { detect: Vec<String> },
    /// Detect optionally (won't fail if none exist)
    DetectOptional { detect_optional: Vec<String> },
}

/// Version source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSource {
    /// Read version from file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,

    /// Pattern to extract version from file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    /// Command to run to get version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Whether to parse version numbers from command output
    #[serde(default = "default_parse_version")]
    pub parse_version: bool,
}

fn default_parse_version() -> bool {
    true
}

/// Complete package manager configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageManagerConfig {
    /// Package manager family definitions (e.g., "node", "ruby", "python")
    #[serde(default)]
    pub package_managers: HashMap<String, PackageManagerDefinition>,

    /// Version detection sources
    #[serde(default)]
    pub version_sources: HashMap<String, Vec<VersionSource>>,
}

/// Runtime detection result
#[derive(Debug, Clone)]
pub struct DetectedPackageManager {
    /// Family name (e.g., "node")
    pub family: String,
    /// Tool name (e.g., "yarn")
    pub tool: String,
    /// Install command
    pub command: String,
    /// Cache configuration
    pub cache_config: CacheConfig,
}

/// Cache configuration derived from package manager detection
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Cache name
    pub name: String,
    /// Versions for cache key
    pub versions: Vec<String>,
    /// Checksum sources
    pub checksum_sources: Vec<String>,
    /// Paths to cache
    pub paths: Vec<String>,
}
