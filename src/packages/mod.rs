pub mod deduplicator;
pub mod detector_dynamic;
pub mod installer;

// Re-export the dynamic detection types
pub use crate::models::{CacheConfig, DetectedPackageManager, PackageManagerConfig};
pub use detector_dynamic::DynamicPackageDetector;
