use crate::models::DetectedPackageManager;
use crate::models::job::Step;
use serde_yaml::{Mapping, Value};

/// Generates installation steps for packages
pub struct PackageInstaller;

impl PackageInstaller {
    /// Generate the installation steps for a detected package manager
    pub fn generate_install_steps(detected: &DetectedPackageManager) -> Vec<Step> {
        vec![
            // Only add install command - checkout is handled by cigen
            Self::create_install_step(&detected.command),
        ]
    }

    /// Generate only the restore cache step (for read-only access)
    pub fn generate_restore_only_step(_cache_name: &str) -> Step {
        // This will be handled by the cache system
        Step(Value::String("checkout".to_string()))
    }

    pub fn create_install_step(command: &str) -> Step {
        let mut step_map = Mapping::new();
        let mut run_config = Mapping::new();
        run_config.insert(
            Value::String("name".to_string()),
            Value::String("Install packages".to_string()),
        );
        run_config.insert(
            Value::String("command".to_string()),
            Value::String(command.to_string()),
        );
        step_map.insert(Value::String("run".to_string()), Value::Mapping(run_config));
        Step(Value::Mapping(step_map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CacheConfig, DetectedPackageManager};

    fn create_test_detected_npm() -> DetectedPackageManager {
        DetectedPackageManager {
            family: "node".to_string(),
            tool: "npm".to_string(),
            command: "npm ci".to_string(),
            cache_config: CacheConfig {
                name: "node_cache".to_string(),
                versions: vec!["node".to_string()],
                checksum_sources: vec!["package.json".to_string(), "package-lock.json".to_string()],
                paths: vec!["node_modules".to_string()],
            },
        }
    }

    fn create_test_detected_bundler() -> DetectedPackageManager {
        DetectedPackageManager {
            family: "ruby".to_string(),
            tool: "bundler".to_string(),
            command: "bundle install --deployment".to_string(),
            cache_config: CacheConfig {
                name: "ruby_cache".to_string(),
                versions: vec!["ruby".to_string(), "bundler".to_string()],
                checksum_sources: vec!["Gemfile".to_string(), "Gemfile.lock".to_string()],
                paths: vec!["vendor/bundle".to_string(), ".bundle".to_string()],
            },
        }
    }

    #[test]
    fn test_generate_npm_install_steps() {
        let detected = create_test_detected_npm();
        let steps = PackageInstaller::generate_install_steps(&detected);

        assert_eq!(steps.len(), 1);

        // Check install step
        let install_step = &steps[0].0;
        let install_map = install_step.as_mapping().unwrap();
        assert!(install_map.contains_key("run"));
        let run_config = install_map.get("run").unwrap().as_mapping().unwrap();
        assert_eq!(
            run_config.get("command").unwrap().as_str().unwrap(),
            "npm ci"
        );
    }

    #[test]
    fn test_generate_bundler_install_steps() {
        let detected = create_test_detected_bundler();
        let steps = PackageInstaller::generate_install_steps(&detected);

        assert_eq!(steps.len(), 1);

        // Check install command is correct
        let install_step = &steps[0].0;
        let install_map = install_step.as_mapping().unwrap();
        let run_config = install_map.get("run").unwrap().as_mapping().unwrap();
        assert_eq!(
            run_config.get("command").unwrap().as_str().unwrap(),
            "bundle install --deployment"
        );
    }

    #[test]
    fn test_generate_restore_only() {
        let step = PackageInstaller::generate_restore_only_step("node_modules");

        // For now, just returns checkout
        assert_eq!(step.0.as_str().unwrap(), "checkout");
    }
}
