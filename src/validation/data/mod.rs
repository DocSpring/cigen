//! Data-level validation with span tracking for precise error reporting

mod cache_validator;
pub mod error;
mod reference_validator;
mod requires_validator;
mod service_validator;
mod source_files_validator;
pub mod span_finder;
mod validator;

pub use reference_validator::ReferenceValidator;
pub use validator::DataValidator;
