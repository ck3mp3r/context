// Job queue module - generic, extensible background job system
//
// Design:
// - Enum-based static dispatch (no trait objects, better performance)
// - JobHandler enum: all job types (Open/Closed via adding variants)
// - JobQueue: in-memory state management (Single Responsibility)
// - JobExecutor: async task spawning (Single Responsibility)

pub mod executor;
pub mod handler;
pub mod queue;

pub use executor::JobExecutor;
pub use handler::{JobError, JobHandler, JobParams, JobResult};
pub use queue::{JobQueue, JobStatus, Status};
