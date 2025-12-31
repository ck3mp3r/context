use serde::{Deserialize, Serialize};

// Re-export common types from shared db models
#[cfg(all(feature = "backend", not(feature = "frontend")))]
pub use crate::db::models::NoteType;

// For frontend builds, define NoteType locally
#[cfg(any(feature = "frontend", not(feature = "backend")))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NoteType {
    Manual,
    ArchivedTodo,
}

/// Project response from API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Repository response from API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Repo {
    pub id: String,
    pub remote: String,
    pub path: Option<String>,
    pub tags: Vec<String>,
    pub project_ids: Vec<String>,
    pub created_at: String,
}

/// Task list response from API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskList {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub external_ref: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub repo_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Task response from API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: String,
    pub list_id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: Option<i32>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: String,
}

/// Task statistics for a task list
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskStats {
    pub list_id: String,
    pub total: usize,
    pub backlog: usize,
    pub todo: usize,
    pub in_progress: usize,
    pub review: usize,
    pub done: usize,
    pub cancelled: usize,
}

/// Note response from API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub note_type: NoteType,
    pub tags: Vec<String>,
    pub project_ids: Vec<String>,
    pub repo_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

/// API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub details: Option<String>,
}
