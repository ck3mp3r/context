//! MCP tool implementations
//!
//! This module contains tool handlers organized by entity type.
//! Each module follows Single Responsibility Principle (SRP).

mod notes;
pub mod projects;
#[cfg(test)]
mod projects_test;
pub mod repos;
#[cfg(test)]
mod repos_test;
pub mod task_lists;
#[cfg(test)]
mod task_lists_test;
mod tasks;

pub use notes::NoteTools;
pub use projects::ProjectTools;
pub use repos::RepoTools;
pub use task_lists::TaskListTools;
pub use tasks::TaskTools;
