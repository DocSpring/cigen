/// CIGen schema types for cigen.yml
///
/// This module defines the data structures for parsing and validating cigen.yml configuration files.
mod command;
mod config;
mod job;
mod step;
mod workflow;

pub use command::{CommandDefinition, CommandParameter};
pub use config::{CacheDefinition, CigenConfig, ProjectConfig, RunnerDefinition};
pub use job::{Job, JobMatrix, JobTrigger, MatrixDimension, PackageSpec, SkipConditions};
pub use step::{
    Artifact, RestoreCacheDefinition, RunStepOptions, SaveCacheDefinition, Step, UsesStep,
};
pub use workflow::{StageDefinition, WorkflowCondition, WorkflowConditionKind, WorkflowConfig};
