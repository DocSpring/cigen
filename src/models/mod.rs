pub mod command;
pub mod config;
pub mod job;
pub mod package_managers;

// Re-export commonly used types
pub use command::Command;
pub use config::{
    Cache, CacheBackend, CacheBackendConfig, CacheDefinition, Config, DefaultCacheConfig,
    DockerAuth, DockerConfig, DockerImageConfig, OutputConfig, ParameterConfig, PathOrDetect,
    Service, VersionSource, WorkflowConfig,
};
pub use job::Job;
pub use package_managers::{
    CacheConfig, ChecksumSource, DetectedPackageManager, PackageManagerConfig,
    PackageManagerDefinition, PackageManagerDetection,
};

// Config loaders for different schema versions
pub mod loaders;

pub use loaders::ConfigLoader;

#[cfg(test)]
mod tests;
