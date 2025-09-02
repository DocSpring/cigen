pub mod command;
pub mod config;
pub mod job;

// Re-export commonly used types
pub use command::Command;
pub use config::{
    Cache, CacheBackend, CacheBackendConfig, CacheDefinition, Config, DefaultCacheConfig,
    DockerAuth, DockerConfig, DockerImageConfig, OutputConfig, ParameterConfig, PathOrDetect,
    Service, VersionSource,
};
pub use job::Job;

// Config loaders for different schema versions
pub mod loaders;

pub use loaders::ConfigLoader;

#[cfg(test)]
mod tests;
