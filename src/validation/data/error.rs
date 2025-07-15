use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic, Clone)]
#[error("{message}")]
pub struct DataValidationError {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("here")]
    pub bad_bit: SourceSpan,
    pub message: String,
}

impl DataValidationError {
    pub fn new(file_path: &str, content: String, span: SourceSpan, message: String) -> Self {
        // Add a space before the path to make it clickable in iTerm2
        let padded_path = format!(" {file_path}");
        Self {
            src: NamedSource::new(padded_path, content),
            bad_bit: span,
            message,
        }
    }
}
