//! SQLite implementation of the database traits.
//!
//! This module provides a SQLite-backed implementation of the repository
//! traits defined in the parent module.

mod connection;
mod helpers;
mod note;
mod project;
mod repo;
mod task;
mod task_list;

#[cfg(test)]
mod connection_test;
#[cfg(test)]
mod critical_tests;
#[cfg(test)]
mod note_test;
#[cfg(test)]
mod project_test;
#[cfg(test)]
mod repo_test;
#[cfg(test)]
mod task_test;

pub use connection::SqliteDatabase;
pub use note::SqliteNoteRepository;
pub use project::SqliteProjectRepository;
pub use repo::SqliteRepoRepository;
pub use task::SqliteTaskRepository;
pub use task_list::SqliteTaskListRepository;
