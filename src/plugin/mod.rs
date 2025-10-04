pub mod discovery;
pub mod manager;
/// Plugin system for CIGen
///
/// This module implements the plugin architecture that allows CIGen to be extended
/// with providers (CircleCI, GitHub Actions, Buildkite) and modules (language support,
/// caching, etc.) as separate processes communicating via gRPC.
pub mod protocol;

// Re-export commonly used types
pub use manager::PluginManager;
pub use protocol::{
    plugin_client::PluginClient,
    plugin_server::{Plugin, PluginServer},
};
