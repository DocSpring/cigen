use crate::models::Config;
use std::collections::HashMap;

/// Resolve a docker image reference to the actual image string for the given architecture
pub fn resolve_docker_image(
    image_ref: &str,
    architecture: Option<&str>,
    config: &Config,
) -> Result<String, String> {
    // If it's already a full image reference (contains ':' or '/'), return as-is
    if image_ref.contains(':') || image_ref.contains('/') {
        return Ok(image_ref.to_string());
    }

    // Look up the image in docker_images configuration
    let docker_images = config.docker_images.as_ref().ok_or_else(|| {
        format!(
            "Docker image '{}' not found - no docker_images configuration",
            image_ref
        )
    })?;

    let image_config = docker_images.get(image_ref).ok_or_else(|| {
        format!(
            "Docker image '{}' not found in docker_images configuration",
            image_ref
        )
    })?;

    // If we have an architecture and architecture-specific images are defined
    if let (Some(arch), Some(arch_images)) = (architecture, &image_config.architectures)
        && let Some(arch_image) = arch_images.get(arch)
    {
        return Ok(arch_image.clone());
    }

    // Fall back to default image
    Ok(image_config.default.clone())
}

/// Generate CircleCI docker image anchors from docker_images configuration
pub fn generate_docker_anchors(config: &Config) -> HashMap<String, String> {
    let mut anchors = HashMap::new();

    if let Some(docker_images) = &config.docker_images {
        for (name, image_config) in docker_images {
            // Generate anchor for default image
            let anchor_name = format!("{}_image", name);
            anchors.insert(anchor_name, image_config.default.clone());

            // Generate anchors for architecture-specific images
            if let Some(arch_images) = &image_config.architectures {
                for (arch, image) in arch_images {
                    let arch_anchor_name = format!("{}_image_{}", name, arch);
                    anchors.insert(arch_anchor_name, image.clone());
                }
            }
        }
    }

    anchors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DockerImageConfig;
    use std::collections::HashMap;

    fn create_test_config() -> Config {
        let mut docker_images = HashMap::new();

        // Ruby image with architecture variants
        let mut ruby_architectures = HashMap::new();
        ruby_architectures.insert("amd64".to_string(), "cimg/ruby:3.3.5".to_string());
        ruby_architectures.insert("arm64".to_string(), "cimg/ruby:3.3.5-arm64".to_string());

        docker_images.insert(
            "ruby".to_string(),
            DockerImageConfig {
                default: "cimg/ruby:3.3.5".to_string(),
                architectures: Some(ruby_architectures),
            },
        );

        // Node image without architecture variants
        docker_images.insert(
            "node".to_string(),
            DockerImageConfig {
                default: "cimg/node:18.17.1".to_string(),
                architectures: None,
            },
        );

        Config {
            docker_images: Some(docker_images),
            ..Default::default()
        }
    }

    #[test]
    fn test_resolve_docker_image_with_architecture() {
        let config = create_test_config();

        let result = resolve_docker_image("ruby", Some("arm64"), &config).unwrap();
        assert_eq!(result, "cimg/ruby:3.3.5-arm64");

        let result = resolve_docker_image("ruby", Some("amd64"), &config).unwrap();
        assert_eq!(result, "cimg/ruby:3.3.5");
    }

    #[test]
    fn test_resolve_docker_image_fallback_to_default() {
        let config = create_test_config();

        // Unknown architecture falls back to default
        let result = resolve_docker_image("ruby", Some("unknown"), &config).unwrap();
        assert_eq!(result, "cimg/ruby:3.3.5");

        // No architecture specified uses default
        let result = resolve_docker_image("ruby", None, &config).unwrap();
        assert_eq!(result, "cimg/ruby:3.3.5");

        // Image without architecture variants uses default
        let result = resolve_docker_image("node", Some("arm64"), &config).unwrap();
        assert_eq!(result, "cimg/node:18.17.1");
    }

    #[test]
    fn test_resolve_docker_image_full_reference() {
        let config = create_test_config();

        // Full image references are returned as-is
        let result = resolve_docker_image("postgres:14.9", None, &config).unwrap();
        assert_eq!(result, "postgres:14.9");

        let result = resolve_docker_image("cimg/postgres:14.9", Some("arm64"), &config).unwrap();
        assert_eq!(result, "cimg/postgres:14.9");
    }

    #[test]
    fn test_resolve_docker_image_not_found() {
        let config = create_test_config();

        let result = resolve_docker_image("nonexistent", None, &config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("not found in docker_images configuration")
        );
    }

    #[test]
    fn test_generate_docker_anchors() {
        let config = create_test_config();
        let anchors = generate_docker_anchors(&config);

        // Check default anchors
        assert_eq!(anchors.get("ruby_image").unwrap(), "cimg/ruby:3.3.5");
        assert_eq!(anchors.get("node_image").unwrap(), "cimg/node:18.17.1");

        // Check architecture-specific anchors
        assert_eq!(anchors.get("ruby_image_amd64").unwrap(), "cimg/ruby:3.3.5");
        assert_eq!(
            anchors.get("ruby_image_arm64").unwrap(),
            "cimg/ruby:3.3.5-arm64"
        );

        // Node doesn't have architecture variants, so no arch-specific anchors
        assert!(!anchors.contains_key("node_image_amd64"));
    }
}
