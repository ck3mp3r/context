//! V1 API handlers.

mod notes;
mod projects;
mod repos;
#[cfg(test)]
#[path = "../../../test/api/v1/restore.rs"]
mod restore_test;
mod skills;
mod sync;
#[cfg(test)]
#[path = "../../../test/api/v1/sync.rs"]
mod sync_test;
mod task_lists;
mod tasks;
#[cfg(test)]
#[path = "../../../test/api/v1/tasks.rs"]
mod tasks_test;

pub use notes::*;
pub use projects::*;
pub use repos::*;
pub use skills::*;
pub use sync::*;
pub use task_lists::*;
pub use tasks::*;
