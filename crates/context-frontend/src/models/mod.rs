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

/// Archive state assumed when a response omits the field (older servers).
fn default_note_status() -> String {
    "active".to_string()
}

/// Note response from API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    /// Archive state: "active" or "archived"
    #[serde(default = "default_note_status")]
    pub status: String,
    /// Timestamp when the note was archived
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
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
    /// Whether the skill's cache directory exists
    #[serde(default)]
    pub enabled: bool,
    /// Source the skill was imported from (absent when unknown)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Subpath inside the source that holds the skill (absent when unknown)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_subpath: Option<String>,
}

/// Settings as stored in `config.toml`: an absent field carries no override.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SettingsValues {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub verbosity: Option<u8>,
    pub enable_docs: Option<bool>,
    pub skills_dir: Option<String>,
    pub start_at_login: Option<bool>,
    pub auto_update: Option<bool>,
    pub update_channel: Option<String>,
}

/// Built-in defaults for every settings field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingsDefaults {
    pub host: String,
    pub port: u16,
    pub verbosity: u8,
    pub enable_docs: bool,
    pub skills_dir: String,
    pub start_at_login: bool,
    pub auto_update: bool,
    pub update_channel: String,
}

/// Update state as `GET /api/v1/update/status` reports it.
///
/// Every optional field is absent from the response when it carries no value.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateStatusPayload {
    pub current_version: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_is_prerelease: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Sync state as `GET /api/v1/sync/status` reports it.
///
/// The body is the server's `SyncResponse` envelope: `status` is `"success"` when sync is
/// initialized and `"not_initialized"` before the first `init`, which is the flag the app reads to
/// decide whether to render the sync surface at all.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncResponseEnvelope {
    pub status: String,
    pub message: String,
    pub data: Option<SyncStatusData>,
}

/// Payload the status envelope carries.
///
/// An uninitialized status carries `initialized: false` and nothing else of value. An initialized
/// one carries the remote, the last sync time, and whether sync is enabled on this machine; the
/// setup envelope omits `enabled` entirely, so the field is optional. `git`, the two count pairs,
/// and the tombstone count are absent when the status read could not produce them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncStatusData {
    pub initialized: bool,
    pub enabled: Option<bool>,
    pub remote_url: Option<String>,
    pub last_synced_at: Option<String>,
    pub git: Option<SyncGitState>,
    pub database: Option<SyncCounts>,
    pub sync_files: Option<SyncCounts>,
    pub tombstones: Option<SyncCounts>,
}

/// Working-tree state of the sync directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncGitState {
    pub clean: bool,
    pub status: String,
}

/// Row count for one side of sync: the server sends the per-entity counts plus their total.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncCounts {
    pub total: u64,
}

/// Outcome of one manual update check that reported no state of its own.
///
/// A settled `idle` state renders nothing and a rejected call renders nothing either, so the pill
/// cannot tell a check that found nothing from one that failed. The settings page writes one of
/// these so the pill can say which happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckOutcome {
    /// The trigger or a status read failed; the text is the detail to show.
    Failed(String),
    /// The check finished and no newer release is published.
    NoUpdate,
}

/// Settings the running server process started with.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingsRunning {
    pub host: String,
    pub port: u16,
    pub verbosity: u8,
    pub enable_docs: bool,
    pub host_fixed: bool,
    pub skills_dir: String,
}

/// Response of the settings endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingsPayload {
    pub values: SettingsValues,
    pub defaults: SettingsDefaults,
    pub running: SettingsRunning,
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
    NoteCreated {
        note_id: String,
    },
    NoteUpdated {
        note_id: String,
    },
    NoteDeleted {
        note_id: String,
    },

    // Skills
    SkillCreated {
        skill_id: String,
    },
    SkillUpdated {
        skill_id: String,
    },
    SkillDeleted {
        skill_id: String,
    },

    // Projects
    ProjectCreated {
        project_id: String,
    },
    ProjectUpdated {
        project_id: String,
    },
    ProjectDeleted {
        project_id: String,
    },

    // Repos
    RepoCreated {
        repo_id: String,
    },
    RepoUpdated {
        repo_id: String,
    },
    RepoDeleted {
        repo_id: String,
    },

    // TaskLists
    TaskListCreated {
        task_list_id: String,
    },
    TaskListUpdated {
        task_list_id: String,
    },
    TaskListDeleted {
        task_list_id: String,
    },

    // Tasks
    TaskCreated {
        task_id: String,
    },
    TaskUpdated {
        task_id: String,
    },
    TaskDeleted {
        task_id: String,
    },

    // Updater
    UpdateStateChanged {
        status: String,
        version: Option<String>,
    },
}
