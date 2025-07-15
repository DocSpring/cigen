mod command;
mod config;
mod data;
mod error_reporter;
mod job;
mod merger;
mod schemas;
mod validator;

#[cfg(test)]
mod tests;

// Re-export the main Validator
pub use validator::Validator;
