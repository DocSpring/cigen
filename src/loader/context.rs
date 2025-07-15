//! Context information for error reporting

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ORIGINAL_DIR: OnceLock<PathBuf> = OnceLock::new();
static CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Initialize the context with the original working directory and config directory
pub fn init_context(original_dir: PathBuf, config_dir: PathBuf) {
    ORIGINAL_DIR.set(original_dir).ok();
    CONFIG_DIR.set(config_dir).ok();
}

/// Convert a path relative to the config directory to be relative to the original directory
pub fn to_original_relative_path(path: &Path) -> PathBuf {
    if let (Some(original), Some(config)) = (ORIGINAL_DIR.get(), CONFIG_DIR.get()) {
        // If the path is already absolute, just return it
        if path.is_absolute() {
            return path.to_path_buf();
        }

        // Make the path absolute relative to the config directory
        let absolute_path = config.join(path);

        // Try to make it relative to the original directory
        if let Ok(relative) = absolute_path.strip_prefix(original) {
            relative.to_path_buf()
        } else {
            // If we can't make it relative, return the absolute path
            absolute_path
        }
    } else {
        // Fallback: return the path as-is
        path.to_path_buf()
    }
}
