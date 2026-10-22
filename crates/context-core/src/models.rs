//! Domain models for the context database.
//!
//! These models are storage-agnostic and represent the core entities
//! used throughout the application.

use serde::{Deserialize, Serialize};

// =============================================================================
// Note Size Limits (to prevent context overflow)
// =============================================================================

/// Warning threshold for note content size (characters).
/// Notes exceeding this size will receive a warning suggesting splitting.
pub const NOTE_WARN_SIZE: usize = 10_000; // ~2,500 tokens

/// Soft maximum for note content size (characters).
/// Notes exceeding this size are allowed but should be split for optimal performance.
pub const NOTE_SOFT_MAX: usize = 50_000; // ~12,500 tokens

/// Hard maximum for note content size (characters).
/// Notes exceeding this size will be rejected.
pub const NOTE_HARD_MAX: usize = 100_000; // ~25,000 tokens

// =============================================================================
// Skill Constants (Agent Skills Specification)
// =============================================================================

/// Maximum length for skill description (Agent Skills standard).
pub const SKILL_DESCRIPTION_MAX: usize = 1_024;

// =============================================================================
// Query Types for Pagination and Sorting
// =============================================================================

/// Sort order for list queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, utoipa::ToSchema)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

// =============================================================================
// Composable Query Types
// =============================================================================

/// Base pagination and sorting options - composed into entity-specific queries.
#[derive(Debug, Clone, Default, serde::Deserialize, utoipa::ToSchema)]
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
    /// Filter by repo ID (projects linked to this repo).
    pub repo_id: Option<String>,
    /// Filter by skill ID (projects linked to this skill).
    pub skill_id: Option<String>,
}

/// Query for Repos - pagination + tags/project filters.
#[derive(Debug, Clone, Default)]
pub struct RepoQuery {
    pub page: PageSort,
    /// Filter by tags (OR logic - matches if ANY tag matches).
    pub tags: Option<Vec<String>>,
    /// Filter by project ID (repos with project_id in project_ids array).
    pub project_id: Option<String>,
    /// Search query for filtering by remote URL or tags (case-insensitive partial match).
    pub search_query: Option<String>,
}

/// Query for TaskLists - pagination + status/tags/project filters.
#[derive(Debug, Clone, Default)]
pub struct TaskListQuery {
    pub page: PageSort,
    /// Filter by status (active, archived).
    pub status: Option<String>,
    /// Filter by tags (OR logic - matches if ANY tag matches).
    pub tags: Option<Vec<String>>,
    /// Filter by project ID.
    pub project_id: Option<String>,
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
    /// Filter by task type: "task" (parent_id IS NULL) or "subtask" (parent_id IS NOT NULL).
    /// Omit to return both tasks and subtasks.
    pub task_type: Option<String>,
}

/// Query for Notes - pagination + tags/project filters.
#[derive(Debug, Clone, Default)]
pub struct NoteQuery {
    pub page: PageSort,
    /// Filter by tags (OR logic - matches if ANY tag matches).
    pub tags: Option<Vec<String>>,
    /// Filter by project ID (notes with project_id in project_ids array).
    pub project_id: Option<String>,
    /// Filter by parent_id (get subnotes of a specific parent note).
    pub parent_id: Option<String>,
    /// Filter by note type: "note" (parent_id IS NULL) or "subnote" (parent_id IS NOT NULL).
    /// Omit to return both parent notes and subnotes.
    pub note_type: Option<String>,
}

/// Query for Skills - pagination + tags/project filters.
#[derive(Debug, Clone, Default)]
pub struct SkillQuery {
    pub page: PageSort,
    /// Filter by tags (OR logic - matches if ANY tag matches).
    pub tags: Option<Vec<String>>,
    /// Filter by project ID (skills with project_id in project_ids array).
    pub project_id: Option<String>,
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
    #[serde(default)]
    pub external_refs: Vec<String>,
    /// Linked repository IDs (M:N relationship via project_repo)
    #[serde(default)]
    pub repo_ids: Vec<Id>,
    /// Linked task list IDs (M:N relationship via project_task_list)
    #[serde(default)]
    pub task_list_ids: Vec<Id>,
    /// Linked note IDs (M:N relationship via project_note)
    #[serde(default)]
    pub note_ids: Vec<Id>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Timestamp when the project was soft-deleted (NULL = not deleted)
    #[serde(default)]
    pub deleted_at: Option<String>,
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
    pub created_at: Option<String>,
    /// Timestamp of the last write; drives incremental sync and the import LWW guard.
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Timestamp when the repo was soft-deleted (NULL = not deleted)
    #[serde(default)]
    pub deleted_at: Option<String>,
}

/// A collection of tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskList {
    pub id: Id,
    pub title: String,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub external_refs: Vec<String>,
    pub status: TaskListStatus,
    /// Linked repository IDs (M:N relationship via task_list_repo)
    #[serde(default)]
    pub repo_ids: Vec<Id>,
    /// Project this task list belongs to (1:N relationship - task list belongs to ONE project, REQUIRED)
    pub project_id: Id,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub archived_at: Option<String>,
    /// Timestamp when the task list was soft-deleted (NULL = not deleted)
    #[serde(default)]
    pub deleted_at: Option<String>,
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
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: Option<i32>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub external_refs: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Timestamp when the task was soft-deleted (NULL = not deleted)
    #[serde(default)]
    pub deleted_at: Option<String>,
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

/// The linear workflow order of active statuses.
const WORKFLOW: [TaskStatus; 5] = [
    TaskStatus::Backlog,
    TaskStatus::Todo,
    TaskStatus::InProgress,
    TaskStatus::Review,
    TaskStatus::Done,
];

impl TaskStatus {
    /// Position of this status in the linear workflow
    /// `backlog -> todo -> in_progress -> review -> done`.
    /// Returns `None` for the side-state `cancelled`.
    pub fn workflow_position(&self) -> Option<usize> {
        WORKFLOW.iter().position(|status| status == self)
    }

    /// Returns `true` for terminal states (`done`, `cancelled`).
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Done | TaskStatus::Cancelled)
    }

    /// Returns `true` for non-terminal states.
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }

    /// The next status in the linear workflow, if any.
    pub fn forward_step(&self) -> Option<TaskStatus> {
        let position = self.workflow_position()?;
        WORKFLOW.get(position + 1).cloned()
    }

    /// The previous status in the linear workflow, if any.
    pub fn backward_step(&self) -> Option<TaskStatus> {
        let position = self.workflow_position()?;
        position
            .checked_sub(1)
            .and_then(|p| WORKFLOW.get(p))
            .cloned()
    }

    /// The status reached by skipping one forward step, if policy allows it.
    /// Only `backlog -> in_progress` and `in_progress -> done` are permitted.
    pub fn expedite_forward(&self) -> Option<TaskStatus> {
        match self {
            TaskStatus::Backlog => Some(TaskStatus::InProgress),
            TaskStatus::InProgress => Some(TaskStatus::Done),
            TaskStatus::Todo | TaskStatus::Review | TaskStatus::Done | TaskStatus::Cancelled => {
                None
            }
        }
    }

    /// All non-terminal statuses, in workflow order.
    pub fn active_states() -> Vec<TaskStatus> {
        WORKFLOW[..4].to_vec()
    }
}

/// Statistics for tasks in a task list, grouped by status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStats {
    pub list_id: Id,
    pub total: usize,
    pub backlog: usize,
    pub todo: usize,
    pub in_progress: usize,
    pub review: usize,
    pub done: usize,
    pub cancelled: usize,
}

/// A log entry recording a task state transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionLog {
    pub id: Id,
    pub task_id: Id,
    pub status: TaskStatus,
    pub transitioned_at: String,
}

/// A persistent markdown note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: Id,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    /// Parent note ID for hierarchical structure (self-referencing FK)
    pub parent_id: Option<Id>,
    /// Manual ordering index within siblings (same parent)
    pub idx: Option<i32>,
    /// Linked repository IDs (M:N relationship via note_repo)
    #[serde(default)]
    pub repo_ids: Vec<Id>,
    /// Linked project IDs (M:N relationship via project_note)
    #[serde(default)]
    pub project_ids: Vec<Id>,
    /// Count of subnotes (children) - computed field, not stored in DB
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnote_count: Option<i32>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Timestamp when the note was soft-deleted (NULL = not deleted)
    #[serde(default)]
    pub deleted_at: Option<String>,
}

/// A skill entity following Agent Skills specification (<https://agentskills.io/specification>).
/// Skills store reusable instructions, scripts, and resources for AI agents.
///
/// The `content` field stores the complete SKILL.md file (YAML frontmatter + Markdown body).
/// LLMs parse the frontmatter themselves - we only extract name/description for DB indexing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub id: Id,
    pub name: String,
    pub description: String,
    /// Full SKILL.md content (YAML frontmatter + Markdown body)
    pub content: String,
    pub tags: Vec<String>,

    #[serde(default)]
    pub project_ids: Vec<Id>,

    /// Script filenames (loaded from skill_attachment where type='script')
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub scripts: Vec<String>,

    /// Reference filenames (loaded from skill_attachment where type='reference')
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub references: Vec<String>,

    /// Asset filenames (loaded from skill_attachment where type='asset')
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub assets: Vec<String>,

    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Timestamp when the skill was soft-deleted (NULL = not deleted)
    #[serde(default)]
    pub deleted_at: Option<String>,
    /// Source URL or path the skill was imported from (NULL = unknown)
    #[serde(default)]
    pub source: Option<String>,
    /// Subpath inside the source that holds the skill (NULL = source root)
    #[serde(default)]
    pub source_subpath: Option<String>,
}

/// A skill attachment (script, reference, or asset file).
/// Part of Agent Skills Phase 2: Attachment Storage & Cache System.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillAttachment {
    pub id: Id,
    pub skill_id: Id,
    /// Attachment type: 'script', 'reference', or 'asset'
    pub type_: String,
    /// Filename (without path, e.g., "deploy.sh", "diagram.png")
    pub filename: String,
    /// Base64-encoded file content
    pub content: String,
    /// SHA256 hash of decoded content (for cache invalidation)
    pub content_hash: String,
    /// MIME type (e.g., "text/x-shellscript", "image/png")
    pub mime_type: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Timestamp when the attachment was soft-deleted (NULL = not deleted)
    #[serde(default)]
    pub deleted_at: Option<String>,
}
