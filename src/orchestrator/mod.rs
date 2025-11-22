/// Job dependency graph and orchestration
mod convert;
mod dag;
mod workflow;

pub use dag::{ConcreteJob, JobDAG};
pub use workflow::{FileFragment, GenerationResult, MergeStrategy, WorkflowOrchestrator};

use crate::schema::CigenConfig;
use std::collections::HashMap;
