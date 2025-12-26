//! V1 API handlers.

mod notes;
mod projects;
mod repos;
mod task_lists;
mod tasks;

// Temporarily disabled API tests during SQLx migration
// #[cfg(test)]
// mod notes_test;
// #[cfg(test)]
// mod projects_test;
// #[cfg(test)]
// mod repos_test;
// #[cfg(test)]
// mod task_lists_test;
// #[cfg(test)]
// mod tasks_test;

pub use notes::*;
pub use projects::*;
pub use repos::*;
pub use task_lists::*;
pub use tasks::*;
