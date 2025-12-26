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

// =============================================================================
// Composable Query Types
// =============================================================================

/// Base pagination and sorting options - composed into entity-specific queries.
#[derive(Debug, Clone, Default)]
pub struct PageSort {
    /// Maximum number of items to return.
    pub limit: Option<usize>,
    /// Number of items to skip.
    pub offset: Option<usize>,
    /// Field to sort by (validated per entity type).
    pub sort_by: Option<String>,
    /// Sort order (ascending or descending).
    pub sort_order: Option<SortOrder>,
}

/// Query for Projects - pagination + tags filter.
#[derive(Debug, Clone, Default)]
pub struct ProjectQuery {
    pub page: PageSort,
    /// Filter by tags (OR logic - matches if ANY tag matches).
    pub tags: Option<Vec<String>>,
}

/// Query for Repos - pagination + tags filter.
#[derive(Debug, Clone, Default)]
pub struct RepoQuery {
    pub page: PageSort,
    /// Filter by tags (OR logic - matches if ANY tag matches).
    pub tags: Option<Vec<String>>,
}

/// Query for TaskLists - pagination + status/tags filters.
#[derive(Debug, Clone, Default)]
pub struct TaskListQuery {
    pub page: PageSort,
    /// Filter by status (active, archived).
    pub status: Option<String>,
    /// Filter by tags (OR logic - matches if ANY tag matches).
    pub tags: Option<Vec<String>>,
}

/// Query for Tasks - pagination + list/parent/status/tags filters.
#[derive(Debug, Clone, Default)]
pub struct TaskQuery {
    pub page: PageSort,
    /// Filter by task list ID (required context for tasks).
    pub list_id: Option<String>,
    /// Filter by parent task ID (for subtasks).
    pub parent_id: Option<String>,
    /// Filter by status (backlog, todo, in_progress, review, done, cancelled).
    pub status: Option<String>,
    /// Filter by tags (OR logic - matches if ANY tag matches).
    pub tags: Option<Vec<String>>,
}

/// Query for Notes - pagination + tags filter.
#[derive(Debug, Clone, Default)]
pub struct NoteQuery {
    pub page: PageSort,
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
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A git repository tracked by the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repo {
    pub id: Id,
    pub remote: String,
    pub path: Option<String>,
    pub tags: Vec<String>,
    /// Linked project IDs (M:N relationship via project_repo)
    #[serde(default)]
    pub project_ids: Vec<Id>,
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
    /// Linked repository IDs (M:N relationship via task_list_repo)
    #[serde(default)]
    pub repo_ids: Vec<Id>,
    /// Linked project IDs (M:N relationship via project_task_list)
    #[serde(default)]
    pub project_ids: Vec<Id>,
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

impl std::fmt::Display for TaskListStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskListStatus::Active => write!(f, "active"),
            TaskListStatus::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for TaskListStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(TaskListStatus::Active),
            "archived" => Ok(TaskListStatus::Archived),
            _ => Err(format!("Unknown task list status: {}", s)),
        }
    }
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
    pub tags: Vec<String>,
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

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Backlog => write!(f, "backlog"),
            TaskStatus::Todo => write!(f, "todo"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Review => write!(f, "review"),
            TaskStatus::Done => write!(f, "done"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "backlog" => Ok(TaskStatus::Backlog),
            "todo" => Ok(TaskStatus::Todo),
            "in_progress" => Ok(TaskStatus::InProgress),
            "review" => Ok(TaskStatus::Review),
            "done" => Ok(TaskStatus::Done),
            "cancelled" => Ok(TaskStatus::Cancelled),
            _ => Err(format!("Invalid TaskStatus: {}", s)),
        }
    }
}

/// A persistent markdown note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: Id,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub note_type: NoteType,
    /// Linked repository IDs (M:N relationship via note_repo)
    #[serde(default)]
    pub repo_ids: Vec<Id>,
    /// Linked project IDs (M:N relationship via project_note)
    #[serde(default)]
    pub project_ids: Vec<Id>,
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

impl std::fmt::Display for NoteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NoteType::Manual => "manual",
            NoteType::ArchivedTodo => "archived_todo",
            NoteType::Scratchpad => "scratchpad",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for NoteType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(NoteType::Manual),
            "archived_todo" => Ok(NoteType::ArchivedTodo),
            "scratchpad" => Ok(NoteType::Scratchpad),
            _ => Err(format!("Invalid note type: {}", s)),
        }
    }
}
