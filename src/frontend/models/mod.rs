use serde::{Deserialize, Serialize};

/// Project response from API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub external_refs: Vec<String>,
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
    pub external_refs: Vec<String>,
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
    pub external_refs: Vec<String>,
    pub created_at: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: Option<String>,
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
    pub tags: Vec<String>,
    /// Parent note ID for hierarchical structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Manual ordering index within siblings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idx: Option<i32>,
    pub project_ids: Vec<String>,
    pub repo_ids: Vec<String>,
    /// Count of subnotes (children) - computed field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnote_count: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

/// Skill response from API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Full SKILL.md content (YAML frontmatter + Markdown body)
    pub content: String,
    pub tags: Vec<String>,
    pub project_ids: Vec<String>,
    /// Script filenames (from skill_attachment where type='script')
    #[serde(default)]
    pub scripts: Vec<String>,
    /// Reference filenames (from skill_attachment where type='reference')
    #[serde(default)]
    pub references: Vec<String>,
    /// Asset filenames (from skill_attachment where type='asset')
    #[serde(default)]
    pub assets: Vec<String>,
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

/// WebSocket update messages from backend
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum UpdateMessage {
    // Notes
    NoteCreated { note_id: String },
    NoteUpdated { note_id: String },
    NoteDeleted { note_id: String },

    // Skills
    SkillCreated { skill_id: String },
    SkillUpdated { skill_id: String },
    SkillDeleted { skill_id: String },

    // Projects
    ProjectCreated { project_id: String },
    ProjectUpdated { project_id: String },
    ProjectDeleted { project_id: String },

    // Repos
    RepoCreated { repo_id: String },
    RepoUpdated { repo_id: String },
    RepoDeleted { repo_id: String },

    // TaskLists
    TaskListCreated { task_list_id: String },
    TaskListUpdated { task_list_id: String },
    TaskListDeleted { task_list_id: String },

    // Tasks
    TaskCreated { task_id: String },
    TaskUpdated { task_id: String },
    TaskDeleted { task_id: String },
}
