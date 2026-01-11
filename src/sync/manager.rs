//! Sync manager - high-level sync operations.
//!
//! Coordinates git operations, export, import, and status checking.

use crate::db::{
    Database, NoteRepository, ProjectRepository, RepoRepository, SyncRepository,
    TaskListRepository, TaskRepository,
};
use miette::Diagnostic;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

use super::{
    export::{ExportError, ExportSummary},
    git::{GitError, GitOps},
    import::{ImportError, ImportSummary},
    paths::get_sync_dir,
    read_jsonl,
};

/// Result of sync initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitResult {
    /// Sync was newly created
    Created,
    /// Sync was already initialized (idempotent operation)
    AlreadyInitialized,
}

/// Errors that can occur during sync operations.
#[derive(Error, Diagnostic, Debug)]
pub enum SyncError {
    #[error("Git error: {0}")]
    #[diagnostic(code(c5t::sync::git))]
    Git(#[from] GitError),

    #[error("Export error: {0}")]
    #[diagnostic(code(c5t::sync::export))]
    Export(#[from] ExportError),

    #[error("Import error: {0}")]
    #[diagnostic(code(c5t::sync::import))]
    Import(#[from] ImportError),

    #[error("Database error: {0}")]
    #[diagnostic(code(c5t::sync::database))]
    Database(#[from] crate::db::DbError),

    #[error("Sync not initialized - run init first")]
    #[diagnostic(code(c5t::sync::not_initialized))]
    NotInitialized,

    #[error("IO error: {0}")]
    #[diagnostic(code(c5t::sync::io))]
    Io(#[from] std::io::Error),
}

/// Sync manager handles all sync operations.
///
/// # Clone Implementation
///
/// SyncManager wraps GitOps in Arc to enable cloning without requiring G: Clone.
/// This allows using non-Clone types like MockGitOps in tests.
/// Clone is implemented manually (not derived) to avoid the G: Clone bound.
pub struct SyncManager<G: GitOps> {
    git: std::sync::Arc<G>,
    sync_dir: PathBuf,
}

// Manual Clone implementation - Arc<G> is Clone even if G is not
impl<G: GitOps> Clone for SyncManager<G> {
    fn clone(&self) -> Self {
        Self {
            git: Arc::clone(&self.git),
            sync_dir: self.sync_dir.clone(),
        }
    }
}

impl<G: GitOps> SyncManager<G> {
    /// Create a new sync manager with the given git operations handler.
    pub fn new(git: G) -> Self {
        Self {
            git: std::sync::Arc::new(git),
            sync_dir: get_sync_dir(),
        }
    }

    /// Create a sync manager with a custom sync directory (for testing).
    pub fn with_sync_dir(git: G, sync_dir: PathBuf) -> Self {
        Self {
            git: std::sync::Arc::new(git),
            sync_dir,
        }
    }

    /// Check if sync is initialized (git repository exists).
    pub fn is_initialized(&self) -> bool {
        self.sync_dir.join(".git").exists()
    }

    /// Initialize sync repository.
    ///
    /// Creates the sync directory, initializes git, and optionally adds a remote.
    /// Idempotent: safe to call multiple times, won't reinitialize existing repos.
    ///
    /// Returns InitResult::Created if newly initialized, InitResult::AlreadyInitialized if already set up.
    pub async fn init(&self, remote_url: Option<String>) -> Result<InitResult, SyncError> {
        tracing::info!("Initializing sync repository at {:?}", self.sync_dir);

        let was_initialized = self.is_initialized();

        // Create sync directory if it doesn't exist
        if !self.sync_dir.exists() {
            tracing::debug!("Creating sync directory");
            std::fs::create_dir_all(&self.sync_dir)?;
        }

        // Initialize git repository only if not already initialized
        if was_initialized {
            tracing::info!("Git repository already initialized, skipping git init");
        } else {
            tracing::debug!("Initializing git repository");
            self.git.init(&self.sync_dir)?;
        }

        // Add remote if provided and not already present
        if let Some(url) = &remote_url {
            match self.git.remote_get_url(&self.sync_dir, "origin") {
                Ok(existing_output) => {
                    let existing_url = String::from_utf8_lossy(&existing_output.stdout)
                        .trim()
                        .to_string();
                    if existing_url == *url {
                        tracing::info!("Remote 'origin' already set to: {}", url);
                    } else {
                        tracing::warn!(
                            existing = %existing_url,
                            new = %url,
                            "Remote 'origin' already exists with different URL, skipping"
                        );
                    }
                }
                Err(_) => {
                    tracing::info!("Adding remote 'origin': {}", url);
                    self.git.add_remote(&self.sync_dir, "origin", url)?;
                }
            }
        }

        let result = if was_initialized {
            InitResult::AlreadyInitialized
        } else {
            InitResult::Created
        };

        tracing::info!(result = ?result, "Sync initialization complete");
        Ok(result)
    }

    /// Export database to JSONL and optionally push to remote.
    ///
    /// # Parameters
    /// - `db`: Database to export from
    /// - `message`: Optional commit message
    /// - `remote`: If true, push to remote after commit (requires remote configured)
    ///
    /// # Idempotency
    /// - ALWAYS exports database to JSONL files
    /// - ALWAYS commits changes (handles "nothing to commit" gracefully)
    /// - If `remote=true`: pushes to remote (safe to run multiple times)
    ///
    /// # Examples
    /// ```ignore
    /// // Local only
    /// manager.export(&db, None, false).await?;
    ///
    /// // Export and push
    /// manager.export(&db, None, true).await?;
    ///
    /// // Retry after network error (idempotent)
    /// manager.export(&db, None, true).await?;
    /// ```
    pub async fn export<D: Database>(
        &self,
        db: &D,
        message: Option<String>,
        remote: bool,
    ) -> Result<ExportSummary, SyncError> {
        tracing::info!(remote = remote, "Starting export operation");

        if !self.is_initialized() {
            tracing::error!("Sync not initialized");
            return Err(SyncError::NotInitialized);
        }

        // Export to JSONL using sync repository
        tracing::info!("Exporting database to JSONL files");
        let summary = db.sync().export_all(&self.sync_dir).await?;
        tracing::info!(
            repos = summary.repos,
            projects = summary.projects,
            task_lists = summary.task_lists,
            tasks = summary.tasks,
            notes = summary.notes,
            "Export complete"
        );

        // Add all JSONL files
        let files = vec![
            "repos.jsonl".to_string(),
            "projects.jsonl".to_string(),
            "lists.jsonl".to_string(),
            "tasks.jsonl".to_string(),
            "notes.jsonl".to_string(),
        ];
        tracing::debug!("Adding files to git");
        self.git.add_files(&self.sync_dir, &files)?;

        // Commit with timestamp-based message if not provided
        let commit_msg = message.unwrap_or_else(|| {
            let now = chrono::Utc::now();
            format!("sync: export at {}", now.format("%Y-%m-%d %H:%M:%S UTC"))
        });

        // Try to commit - if nothing to commit, that's okay (not an error)
        tracing::debug!(message = %commit_msg, "Committing changes");
        match self.git.commit(&self.sync_dir, &commit_msg) {
            Ok(_) => {
                tracing::info!("Changes committed successfully");
                // Push if requested and remote exists
                if remote && self.has_remote()? {
                    tracing::info!("Pushing to remote");
                    self.git.push(&self.sync_dir, "origin", "main")?;
                    tracing::info!("Push complete");
                }
            }
            Err(GitError::NonZeroExit { code: 1, output })
                if output.contains("nothing to commit")
                    || output.contains("nothing added to commit") =>
            {
                tracing::info!("No changes to commit - data already synced");
                // Still push if requested and remote exists (idempotent)
                if remote && self.has_remote()? {
                    tracing::info!("Pushing to remote (no new commits)");
                    self.git.push(&self.sync_dir, "origin", "main")?;
                    tracing::info!("Push complete");
                }
            }
            Err(e) => return Err(e.into()),
        }

        Ok(summary)
    }

    /// Import JSONL to database, optionally pulling from remote first.
    ///
    /// # Parameters
    /// - `db`: Database to import into
    /// - `remote`: If true, pull from remote before import (requires remote configured)
    ///
    /// # Idempotency
    /// - If `remote=true`: pulls from remote FIRST (handles "already up to date" gracefully)
    /// - ALWAYS imports JSONL files to database (upsert behavior - safe to run multiple times)
    ///
    /// # Examples
    /// ```ignore
    /// // Import from local files only
    /// manager.import(&db, false).await?;
    ///
    /// // Pull then import
    /// manager.import(&db, true).await?;
    ///
    /// // Retry after network error (idempotent)
    /// manager.import(&db, true).await?;
    /// ```
    pub async fn import<D: Database>(
        &self,
        db: &D,
        remote: bool,
    ) -> Result<ImportSummary, SyncError> {
        tracing::info!(remote = remote, "Starting import operation");

        if !self.is_initialized() {
            tracing::error!("Sync not initialized");
            return Err(SyncError::NotInitialized);
        }

        // Pull latest changes if requested
        if remote && self.has_remote()? {
            tracing::info!("Pulling latest changes from remote");
            self.git.pull(&self.sync_dir, "origin", "main")?;
            tracing::info!("Pull complete");
        }

        // Import from JSONL using sync repository
        tracing::info!("Importing JSONL files to database");
        let summary = db.sync().import_all(&self.sync_dir).await?;
        tracing::info!(
            repos = summary.repos,
            projects = summary.projects,
            task_lists = summary.task_lists,
            tasks = summary.tasks,
            notes = summary.notes,
            "Import complete"
        );

        Ok(summary)
    }

    /// Get sync status.
    pub async fn status<D: Database>(&self, db: &D) -> Result<SyncStatus, SyncError> {
        if !self.is_initialized() {
            return Ok(SyncStatus {
                initialized: false,
                remote_url: None,
                git_status: None,
                db_counts: None,
                jsonl_counts: None,
            });
        }

        // Get remote URL
        let remote_url = self
            .git
            .remote_get_url(&self.sync_dir, "origin")
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|s| s.trim().to_string());

        // Get git status
        let git_output = self.git.status_porcelain(&self.sync_dir)?;
        let git_status_str = String::from_utf8_lossy(&git_output.stdout);
        let is_clean = git_status_str.trim().is_empty();

        // Count entities in database
        let db_counts = EntityCounts {
            repos: db.repos().list(None).await?.total,
            projects: db.projects().list(None).await?.total,
            task_lists: db.task_lists().list(None).await?.total,
            tasks: db.tasks().list(None).await?.total,
            notes: db.notes().list(None).await?.total,
        };

        // Count entities in JSONL files
        let jsonl_counts = self.count_jsonl_entities().await;

        Ok(SyncStatus {
            initialized: true,
            remote_url,
            git_status: Some(GitStatus {
                clean: is_clean,
                status_output: git_status_str.to_string(),
            }),
            db_counts: Some(db_counts),
            jsonl_counts,
        })
    }

    /// Check if a remote is configured.
    fn has_remote(&self) -> Result<bool, SyncError> {
        match self.git.remote_get_url(&self.sync_dir, "origin") {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Count entities in JSONL files.
    async fn count_jsonl_entities(&self) -> Option<EntityCounts> {
        use crate::db::{Note, Project, Repo, Task, TaskList};

        let repos: Vec<Repo> = read_jsonl(&self.sync_dir.join("repos.jsonl")).ok()?;
        let projects: Vec<Project> = read_jsonl(&self.sync_dir.join("projects.jsonl")).ok()?;
        let task_lists: Vec<TaskList> = read_jsonl(&self.sync_dir.join("lists.jsonl")).ok()?;
        let tasks: Vec<Task> = read_jsonl(&self.sync_dir.join("tasks.jsonl")).ok()?;
        let notes: Vec<Note> = read_jsonl(&self.sync_dir.join("notes.jsonl")).ok()?;

        Some(EntityCounts {
            repos: repos.len(),
            projects: projects.len(),
            task_lists: task_lists.len(),
            tasks: tasks.len(),
            notes: notes.len(),
        })
    }
}

/// Status of the sync system.
#[derive(Debug)]
pub struct SyncStatus {
    pub initialized: bool,
    pub remote_url: Option<String>,
    pub git_status: Option<GitStatus>,
    pub db_counts: Option<EntityCounts>,
    pub jsonl_counts: Option<EntityCounts>,
}

/// Git repository status.
#[derive(Debug)]
pub struct GitStatus {
    pub clean: bool,
    pub status_output: String,
}

/// Entity counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityCounts {
    pub repos: usize,
    pub projects: usize,
    pub task_lists: usize,
    pub tasks: usize,
    pub notes: usize,
}

impl EntityCounts {
    pub fn total(&self) -> usize {
        self.repos + self.projects + self.task_lists + self.tasks + self.notes
    }
}
