//! Template error types with miette integration

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("Template error")]
pub struct TemplateError {
    // The source code is stored as NamedSource for better error display
    #[source_code]
    src: NamedSource<String>,

    // The error span
    #[label("{}", self.reason)]
    span: Option<SourceSpan>,

    // The error reason
    reason: String,

    // Help text for the user
    #[help]
    help: Option<String>,

    // The underlying MiniJinja error for chained error support
    #[source]
    source: Option<minijinja::Error>,
}

impl TemplateError {
    /// Create a new template error from a MiniJinja error
    pub fn from_minijinja_error(
        error: minijinja::Error,
        source: String,
        source_path: &std::path::Path,
    ) -> Self {
        let (span, reason, help) = Self::extract_error_info(&error, &source);

        // Create a named source for better error display
        let src = crate::error_utils::create_named_source(source_path, source.clone());

        Self {
            src,
            span,
            reason,
            help,
            source: Some(error),
        }
    }

    /// Extract error information from MiniJinja error
    fn extract_error_info(
        error: &minijinja::Error,
        source: &str,
    ) -> (Option<SourceSpan>, String, Option<String>) {
        // Get line and column information if available
        let span = if let Some(line) = error.line() {
            // MiniJinja uses 1-based line numbers
            let line_idx = line.saturating_sub(1);

            // Try to get the byte range for more precise error location
            if let Some(range) = error.range() {
                Some(SourceSpan::from(range))
            } else {
                // Fallback: calculate offset from line number
                let offset = Self::line_to_offset(source, line_idx);
                Some(SourceSpan::from(offset))
            }
        } else {
            None
        };

        // Extract error reason with better formatting
        let reason = match error.kind() {
            minijinja::ErrorKind::UndefinedError => {
                // Try to extract the variable name from the error detail
                if let Some(detail) = error.detail() {
                    format!("undefined variable: {detail}")
                } else {
                    "undefined variable".to_string()
                }
            }
            minijinja::ErrorKind::SyntaxError => {
                format!(
                    "syntax error: {}",
                    error.detail().unwrap_or("invalid syntax")
                )
            }
            minijinja::ErrorKind::TemplateNotFound => {
                format!(
                    "template not found: {}",
                    error.detail().unwrap_or("unknown")
                )
            }
            minijinja::ErrorKind::InvalidOperation => {
                format!("invalid operation: {}", error.detail().unwrap_or("unknown"))
            }
            minijinja::ErrorKind::TooManyArguments => {
                format!(
                    "too many arguments: {}",
                    error.detail().unwrap_or("unknown")
                )
            }
            minijinja::ErrorKind::MissingArgument => {
                format!("missing argument: {}", error.detail().unwrap_or("unknown"))
            }
            minijinja::ErrorKind::UnknownFilter => {
                format!("unknown filter: {}", error.detail().unwrap_or("unknown"))
            }
            minijinja::ErrorKind::UnknownFunction => {
                format!("unknown function: {}", error.detail().unwrap_or("unknown"))
            }
            minijinja::ErrorKind::UnknownMethod => {
                format!("unknown method: {}", error.detail().unwrap_or("unknown"))
            }
            minijinja::ErrorKind::BadInclude => {
                format!("bad include: {}", error.detail().unwrap_or("unknown"))
            }
            minijinja::ErrorKind::BadEscape => "bad escape sequence".to_string(),
            minijinja::ErrorKind::CannotUnpack => "cannot unpack value".to_string(),
            minijinja::ErrorKind::CannotDeserialize => "cannot deserialize value".to_string(),
            minijinja::ErrorKind::WriteFailure => "write failure".to_string(),
            _ => error.to_string(),
        };

        // Provide helpful suggestions based on error type
        let help = match error.kind() {
            minijinja::ErrorKind::UndefinedError => Some(
                "Make sure this variable is defined in:\n\
                     • Your vars section in config.yml or config/vars.yml\n\
                     • Environment variables (CIGEN_VAR_<NAME>)\n\
                     • CLI arguments (--var name=value)"
                    .to_string(),
            ),
            minijinja::ErrorKind::SyntaxError => Some(
                "Check the MiniJinja template syntax at https://docs.rs/minijinja/".to_string(),
            ),
            minijinja::ErrorKind::UnknownFilter => Some(
                "Available filters: upper, lower, trim, and more. See MiniJinja docs.".to_string(),
            ),
            minijinja::ErrorKind::UnknownFunction => Some(
                "Available functions: read(filename). Check if you're calling the right function."
                    .to_string(),
            ),
            _ => None,
        };

        (span, reason, help)
    }

    /// Calculate byte offset from line number
    fn line_to_offset(source: &str, target_line: usize) -> usize {
        let mut current_line = 0;

        for (idx, ch) in source.char_indices() {
            if current_line == target_line {
                return idx;
            }

            if ch == '\n' {
                current_line += 1;
            }
        }

        // If we didn't find the line, return the end of the source
        source.len()
    }
}
