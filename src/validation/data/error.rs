use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("Data validation error")]
pub struct DataValidationError {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("here")]
    pub bad_bit: SourceSpan,
    pub message: String,
}

impl DataValidationError {
    pub fn new(file_path: &str, content: String, span: SourceSpan, message: String) -> Self {
        Self {
            src: NamedSource::new(file_path, content),
            bad_bit: span,
            message,
        }
    }
}
