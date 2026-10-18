//! SQLite implementation of the database traits.
//!
//! This module provides a SQLite-backed implementation of the repository
//! traits defined in the parent module.

mod connection;
mod helpers;
mod note;
mod project;
mod repo;
mod skill;
mod sync;
mod task;
mod task_list;
mod transition_log;

#[cfg(test)]
#[path = "../test/connection.rs"]
mod connection_test;
#[cfg(test)]
#[path = "../test/critical.rs"]
mod critical_tests;
#[cfg(test)]
#[path = "../test/helpers.rs"]
mod helpers_test;
#[cfg(test)]
#[path = "../test/note_crud.rs"]
mod note_crud_test;
#[cfg(test)]
#[path = "../test/note_fts.rs"]
mod note_fts_test;
#[cfg(test)]
#[path = "../test/note_read_edit.rs"]
mod note_read_edit_test;
#[cfg(test)]
#[path = "../test/note_subnote.rs"]
mod note_subnote_test;
#[cfg(test)]
#[path = "../test/note.rs"]
mod note_test;
#[cfg(test)]
#[path = "../test/project.rs"]
mod project_test;
#[cfg(test)]
#[path = "../test/repo.rs"]
mod repo_test;
#[cfg(test)]
#[path = "../test/skill.rs"]
mod skill_test;
#[cfg(test)]
#[path = "../test/sync_export.rs"]
mod sync_export_test;
#[cfg(test)]
#[path = "../test/sync_import.rs"]
mod sync_import_test;
#[cfg(test)]
#[path = "../test/sync_roundtrip.rs"]
mod sync_roundtrip_test;
#[cfg(test)]
#[path = "../test/task_crud.rs"]
mod task_crud_test;
#[cfg(test)]
#[path = "../test/task_list.rs"]
mod task_list_test;
#[cfg(test)]
#[path = "../test/task.rs"]
mod task_test;
#[cfg(test)]
#[path = "../test/task_transition.rs"]
mod task_transition_test;
#[cfg(test)]
#[path = "../test/transition_log.rs"]
mod transition_log_test;

pub use connection::SqliteDatabase;
pub use note::SqliteNoteRepository;
pub use project::SqliteProjectRepository;
pub use repo::SqliteRepoRepository;
pub use skill::SqliteSkillRepository;
pub use sync::SqliteSyncRepository;
pub use task::SqliteTaskRepository;
pub use task_list::SqliteTaskListRepository;
pub use transition_log::SqliteTransitionLogRepository;
