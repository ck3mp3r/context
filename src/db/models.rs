//! Domain models for the context database.
//!
//! These models are storage-agnostic and represent the core entities
//! used throughout the application.

use serde::{Deserialize, Serialize};

// =============================================================================
// Query Types for Pagination and Sorting
// =============================================================================

/// Sort order for list queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

/// Query parameters for paginated list operations.
#[derive(Debug, Clone, Default)]
pub struct ListQuery {
    /// Maximum number of items to return.
    pub limit: Option<usize>,
    /// Number of items to skip.
    pub offset: Option<usize>,
    /// Field to sort by (validated per entity type).
    pub sort_by: Option<String>,
    /// Sort order (ascending or descending).
    pub sort_order: Option<SortOrder>,
    /// Filter by tags (OR logic - matches if ANY tag matches).
    pub tags: Option<Vec<String>>,
}

/// Result of a paginated list query.
#[derive(Debug, Clone)]
pub struct ListResult<T> {
    /// The items in this page.
    pub items: Vec<T>,
    /// Total count of all matching items (before pagination).
    pub total: usize,
    /// Limit that was applied.
    pub limit: Option<usize>,
    /// Offset that was applied.
    pub offset: usize,
}

/// 8-character hex ID type used for all entities.
pub type Id = String;

/// A project groups related repositories, task lists, and notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: Id,
    pub title: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A git repository tracked by the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repo {
    pub id: Id,
    pub remote: String,
    pub path: Option<String>,
    pub created_at: String,
}

/// A collection of tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskList {
    pub id: Id,
    pub name: String,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub external_ref: Option<String>,
    pub status: TaskListStatus,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

/// Status of a task list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskListStatus {
    #[default]
    Active,
    Archived,
}

/// An individual work item within a task list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: Id,
    pub list_id: Id,
    pub parent_id: Option<Id>,
    pub content: String,
    pub status: TaskStatus,
    pub priority: Option<i32>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// Status of a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Backlog,
    Todo,
    InProgress,
    Review,
    Done,
    Cancelled,
}

/// A persistent markdown note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: Id,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub note_type: NoteType,
    pub created_at: String,
    pub updated_at: String,
}

/// Type of note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NoteType {
    #[default]
    Manual,
    ArchivedTodo,
    Scratchpad,
}
