//! Job queue module - background job system
//!
//! Pure static dispatch - NO dyn, NO Box, NO trait objects

pub mod executor;
pub mod job;
pub mod queue;
pub mod registry;

pub use executor::JobExecutor;
pub use job::{AnalyzeRepositoryJob, JobError};
pub use queue::{JobQueue, JobStatus, QueueError, Status};
pub use registry::JobRegistry;
