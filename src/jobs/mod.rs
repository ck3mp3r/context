//! Job queue module - pure static dispatch
//!
//! NO dyn, NO Box - registry returns job instances via enum

pub mod executor;
pub mod handlers;
pub mod job_trait;
pub mod queue;
pub mod registry;

pub use executor::JobExecutor;
pub use handlers::AnalyzeRepositoryJob;
pub use job_trait::{Job, JobError};
pub use queue::{JobQueue, JobStatus, QueueError, Status};
pub use registry::{JobInstance, JobRegistry};
