/// CIGen schema types for cigen.yml
///
/// This module defines the data structures for parsing and validating cigen.yml configuration files.
mod config;
mod job;
mod step;

pub use config::CigenConfig;
pub use job::{Job, JobTrigger, MatrixDimension, SkipConditions};
pub use step::{Artifact, RunStepOptions, Step, UsesStep};
