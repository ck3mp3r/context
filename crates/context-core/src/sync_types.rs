//! Sync summary types for export/import operations.
//!
//! These types are shared between the sync module and the database module
//! to break the circular dependency that existed when they were defined
//! in the sync module.

/// Summary of exported/imported entities.
///
/// Tracks counts for each entity type processed during sync operations.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub repos: usize,
    pub projects: usize,
    pub task_lists: usize,
    pub tasks: usize,
    pub transitions: usize,
    pub notes: usize,
    pub skills: usize,
    pub attachments: usize,
}

impl SyncSummary {
    pub fn total(&self) -> usize {
        self.repos
            + self.projects
            + self.task_lists
            + self.tasks
            + self.transitions
            + self.notes
            + self.skills
            + self.attachments
    }
}

/// Type alias for export summary (backward compatibility).
pub type ExportSummary = SyncSummary;

/// Type alias for import summary (backward compatibility).
pub type ImportSummary = SyncSummary;
