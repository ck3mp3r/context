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
// TODO: Re-enable after updating for async - see repositories_test.rs.disabled
// #[cfg(test)]
// mod repositories_test;

pub use connection::SqliteDatabase;
pub use note::SqliteNoteRepository;
pub use project::SqliteProjectRepository;
pub use repo::SqliteRepoRepository;
pub use task::SqliteTaskRepository;
pub use task_list::SqliteTaskListRepository;
