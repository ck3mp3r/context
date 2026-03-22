// Job queue module - generic, extensible background job system
//
// SOLID principles:
// - Job trait: defines execution contract (Open/Closed)
// - JobRegistry: dynamic handler lookup (Dependency Inversion)
// - JobQueue: in-memory state management (Single Responsibility)
// - JobExecutor: async task spawning (Single Responsibility)

pub mod queue;

pub use queue::{JobQueue, JobStatus, Status};
