use crate::models::{Config, DefaultCacheConfig};
use once_cell::sync::Lazy;

// Embed the default cache definitions YAML file at compile time
const DEFAULT_CACHE_DEFINITIONS_YAML: &str = include_str!("cache_definitions.yaml");

// Parse the default config once at startup
pub static DEFAULT_CACHE_CONFIG: Lazy<DefaultCacheConfig> = Lazy::new(|| {
    serde_yaml::from_str(DEFAULT_CACHE_DEFINITIONS_YAML)
        .expect("Failed to parse default cache definitions - this is a bug")
});

/// Merge user config with default config
/// User config takes precedence over defaults
pub fn merge_with_defaults(mut user_config: Config) -> Config {
    let defaults = &*DEFAULT_CACHE_CONFIG;

    // Merge cache_definitions
    if user_config.cache_definitions.is_none() {
        user_config.cache_definitions = Some(defaults.cache_definitions.clone());
    } else if let Some(user_defs) = &mut user_config.cache_definitions {
        // Add default definitions that don't exist in user config
        for (name, def) in &defaults.cache_definitions {
            user_defs.entry(name.clone()).or_insert_with(|| def.clone());
        }
    }

    // Merge version_sources
    if user_config.version_sources.is_none() {
        user_config.version_sources = Some(defaults.version_sources.clone());
    } else if let Some(user_sources) = &mut user_config.version_sources {
        // Add default version sources that don't exist in user config
        for (name, sources) in &defaults.version_sources {
            user_sources
                .entry(name.clone())
                .or_insert_with(|| sources.clone());
        }
    }

    user_config
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CacheDefinition;
    use std::collections::HashMap;

    #[test]
    fn test_default_config_loads() {
        // This will panic if the embedded YAML is invalid
        let _ = &*DEFAULT_CACHE_CONFIG;
    }

    #[test]
    fn test_merge_empty_config() {
        let user_config = Config::default();

        let merged = merge_with_defaults(user_config);

        // Should have default cache definitions
        assert!(merged.cache_definitions.is_some());
        let cache_defs = merged.cache_definitions.unwrap();
        assert!(cache_defs.contains_key("gems"));
        assert!(cache_defs.contains_key("node_modules"));
        assert!(cache_defs.contains_key("pip"));

        // Should have default version sources
        assert!(merged.version_sources.is_some());
        let version_sources = merged.version_sources.unwrap();
        assert!(version_sources.contains_key("ruby"));
        assert!(version_sources.contains_key("node"));
        assert!(version_sources.contains_key("python"));
    }

    #[test]
    fn test_merge_preserves_user_config() {
        let mut user_cache_defs = HashMap::new();
        user_cache_defs.insert(
            "gems".to_string(),
            CacheDefinition {
                key: None,
                versions: None,
                checksum_sources: None,
                paths: None,
                backend: Some("custom".to_string()),
                config: None,
            },
        );

        let user_config = Config {
            cache_definitions: Some(user_cache_defs),
            ..Config::default()
        };

        let merged = merge_with_defaults(user_config);

        // User's gems definition should be preserved
        let cache_defs = merged.cache_definitions.unwrap();
        let gems_def = cache_defs.get("gems").unwrap();
        assert_eq!(gems_def.backend, Some("custom".to_string()));

        // Default definitions should still be added
        assert!(cache_defs.contains_key("node_modules"));
        assert!(cache_defs.contains_key("pip"));
    }
}
