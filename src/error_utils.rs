//! Shared error utilities

use miette::NamedSource;
use std::path::Path;

/// Toggle this to add spaces for iTerm2 clickability
const ADD_SPACES_FOR_ITERM: bool = true;

/// Format a file path for error display
///
/// When ADD_SPACES_FOR_ITERM is true, adds a space before the path
/// to make it clickable in iTerm2.
pub fn format_error_path(path: &Path) -> String {
    let display_path = crate::loader::context::to_original_relative_path(path);
    let path_str = display_path.display().to_string();

    let formatted_path = path_str;

    if ADD_SPACES_FOR_ITERM {
        format!(" {formatted_path}")
    } else {
        formatted_path
    }
}

/// Create a NamedSource with proper formatting for error display
pub fn create_named_source(path: &Path, content: String) -> NamedSource<String> {
    let formatted_path = format_error_path(path);
    NamedSource::new(formatted_path, content)
}
