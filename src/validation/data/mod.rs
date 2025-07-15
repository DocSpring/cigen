//! Data-level validation with span tracking for precise error reporting

pub mod error;
pub mod span_finder;
mod validator;

pub use validator::DataValidator;
