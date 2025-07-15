//! Data-level validation with span tracking for precise error reporting

mod error;
mod span_finder;
mod validator;

pub use validator::DataValidator;
