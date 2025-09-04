use crate::models::{
    CacheConfig, ChecksumSource, DetectedPackageManager, PackageManagerConfig,
    PackageManagerDefinition,
};
use miette::Result;
use std::path::Path;

/// Dynamic package manager detector that uses YAML configuration
pub struct DynamicPackageDetector {
    project_root: String,
    config: PackageManagerConfig,
}

impl DynamicPackageDetector {
    pub fn new(project_root: &str, config: PackageManagerConfig) -> Self {
        Self {
            project_root: project_root.to_string(),
            config,
        }
    }

    pub fn project_root(&self) -> &str {
        &self.project_root
    }
    pub fn config(&self) -> &PackageManagerConfig {
        &self.config
    }

    /// Detect package manager for a given family (e.g., "node", "ruby")
    pub fn detect_package_manager(&self, family: &str) -> Result<DetectedPackageManager> {
        let definition = self
            .config
            .package_managers
            .get(family)
            .ok_or_else(|| miette::miette!("Unknown package manager family: {family}"))?;

        let root = Path::new(&self.project_root);

        // Try each detection rule in order
        for detection in &definition.detect {
            let lockfile_path = root.join(&detection.lockfile);
            if lockfile_path.exists() {
                // Found the lockfile, this is the package manager to use
                return Ok(DetectedPackageManager {
                    family: family.to_string(),
                    tool: detection.name.clone(),
                    command: detection.command.clone(),
                    cache_config: self.build_cache_config(family, definition)?,
                });
            }
        }

        miette::bail!(
            "No package manager detected for {family} in {}",
            self.project_root
        )
    }

    fn build_cache_config(
        &self,
        family: &str,
        definition: &PackageManagerDefinition,
    ) -> Result<CacheConfig> {
        // Resolve checksum sources
        let mut checksum_sources = Vec::new();
        let root = Path::new(&self.project_root);

        for source in &definition.checksum_sources {
            match source {
                ChecksumSource::File(file) => {
                    checksum_sources.push(file.clone());
                }
                ChecksumSource::Detect { detect } => {
                    // Find the first file that exists
                    for file in detect {
                        if root.join(file).exists() {
                            checksum_sources.push(file.clone());
                            break;
                        }
                    }
                }
                ChecksumSource::DetectOptional { detect_optional } => {
                    // Add all files that exist
                    for file in detect_optional {
                        if root.join(file).exists() {
                            checksum_sources.push(file.clone());
                        }
                    }
                }
            }
        }

        // Resolve versions (would need to implement version detection here)
        let versions = definition.versions.clone(); // Simplified for now

        Ok(CacheConfig {
            name: format!("{family}_cache"),
            versions,
            checksum_sources,
            paths: definition.cache_paths.clone(),
        })
    }

    /// Get default package manager configuration with built-in definitions
    pub fn default_config() -> PackageManagerConfig {
        let mut config = PackageManagerConfig::default();

        // Load package manager definitions
        let package_managers = [
            (
                "node",
                include_str!("config_templates/package_managers/node.yml"),
            ),
            (
                "ruby",
                include_str!("config_templates/package_managers/ruby.yml"),
            ),
            (
                "python",
                include_str!("config_templates/package_managers/python.yml"),
            ),
            (
                "go",
                include_str!("config_templates/package_managers/go.yml"),
            ),
            (
                "rust",
                include_str!("config_templates/package_managers/rust.yml"),
            ),
            (
                "java",
                include_str!("config_templates/package_managers/java.yml"),
            ),
            (
                "dotnet",
                include_str!("config_templates/package_managers/dotnet.yml"),
            ),
        ];

        for (name, yaml_content) in package_managers {
            let definition: PackageManagerDefinition = serde_yaml::from_str(yaml_content)
                .unwrap_or_else(|e| panic!("Invalid {name} package manager definition: {e}"));
            config.package_managers.insert(name.to_string(), definition);
        }

        // Load version sources
        let version_sources = [
            (
                "node",
                include_str!("config_templates/version_sources/node.yml"),
            ),
            (
                "ruby",
                include_str!("config_templates/version_sources/ruby.yml"),
            ),
            (
                "bundler",
                include_str!("config_templates/version_sources/bundler.yml"),
            ),
            (
                "python",
                include_str!("config_templates/version_sources/python.yml"),
            ),
            (
                "go",
                include_str!("config_templates/version_sources/go.yml"),
            ),
            (
                "rustc",
                include_str!("config_templates/version_sources/rustc.yml"),
            ),
            (
                "cargo",
                include_str!("config_templates/version_sources/cargo.yml"),
            ),
            (
                "java",
                include_str!("config_templates/version_sources/java.yml"),
            ),
            (
                "dotnet",
                include_str!("config_templates/version_sources/dotnet.yml"),
            ),
        ];

        for (name, yaml_content) in version_sources {
            let sources: Vec<crate::models::package_managers::VersionSource> =
                serde_yaml::from_str(yaml_content)
                    .unwrap_or_else(|e| panic!("Invalid {name} version sources: {e}"));
            config.version_sources.insert(name.to_string(), sources);
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChecksumSource, PackageManagerDefinition, PackageManagerDetection};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::tempdir;

    fn create_test_config() -> PackageManagerConfig {
        let mut package_managers = HashMap::new();

        let node_definition = PackageManagerDefinition {
            versions: vec!["node".to_string()],
            detect: vec![
                PackageManagerDetection {
                    name: "npm".to_string(),
                    lockfile: "package-lock.json".to_string(),
                    command: "npm ci".to_string(),
                    condition: None,
                },
                PackageManagerDetection {
                    name: "yarn".to_string(),
                    lockfile: "yarn.lock".to_string(),
                    command: "yarn install --frozen-lockfile".to_string(),
                    condition: None,
                },
            ],
            checksum_sources: vec![
                ChecksumSource::File("package.json".to_string()),
                ChecksumSource::Detect {
                    detect: vec!["package-lock.json".to_string(), "yarn.lock".to_string()],
                },
            ],
            cache_paths: vec!["node_modules".to_string()],
        };

        package_managers.insert("node".to_string(), node_definition);

        PackageManagerConfig {
            package_managers,
            version_sources: HashMap::new(),
        }
    }

    #[test]
    fn test_detect_npm() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        // Create package-lock.json
        fs::write(path.join("package-lock.json"), "{}").unwrap();
        fs::write(path.join("package.json"), "{}").unwrap();

        let config = create_test_config();
        let detector = DynamicPackageDetector::new(path.to_str().unwrap(), config);
        let detected = detector.detect_package_manager("node").unwrap();

        assert_eq!(detected.family, "node");
        assert_eq!(detected.tool, "npm");
        assert_eq!(detected.command, "npm ci");
    }

    #[test]
    fn test_detect_yarn() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        // Create yarn.lock
        fs::write(path.join("yarn.lock"), "").unwrap();
        fs::write(path.join("package.json"), "{}").unwrap();

        let config = create_test_config();
        let detector = DynamicPackageDetector::new(path.to_str().unwrap(), config);
        let detected = detector.detect_package_manager("node").unwrap();

        assert_eq!(detected.family, "node");
        assert_eq!(detected.tool, "yarn");
        assert_eq!(detected.command, "yarn install --frozen-lockfile");
    }

    #[test]
    fn test_no_lockfile_found() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        let config = create_test_config();
        let detector = DynamicPackageDetector::new(path.to_str().unwrap(), config);
        let result = detector.detect_package_manager("node");

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No package manager detected")
        );
    }
}
