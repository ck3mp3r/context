//! Sync manager - high-level sync operations.
//!
//! Coordinates git operations, export, import, and status checking.

use crate::db::{
    Database, NoteRepository, ProjectRepository, RepoRepository, TaskListRepository, TaskRepository,
};
use std::path::PathBuf;
use thiserror::Error;

use super::{
    export::{ExportError, ExportSummary, export_all},
    git::{GitError, GitOps},
    import::{ImportError, ImportSummary, import_all},
    paths::get_sync_dir,
    read_jsonl,
};

/// Errors that can occur during sync operations.
#[derive(Error, Debug)]
pub enum SyncError {
    #[error("Git error: {0}")]
    Git(#[from] GitError),

    #[error("Export error: {0}")]
    Export(#[from] ExportError),

    #[error("Import error: {0}")]
    Import(#[from] ImportError),

    #[error("Database error: {0}")]
    Database(#[from] crate::db::DbError),

    #[error("Sync not initialized - run init first")]
    NotInitialized,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Sync manager handles all sync operations.
pub struct SyncManager<G: GitOps> {
    git: G,
    sync_dir: PathBuf,
}

impl<G: GitOps> SyncManager<G> {
    /// Create a new sync manager with the given git operations handler.
    pub fn new(git: G) -> Self {
        Self {
            git,
            sync_dir: get_sync_dir(),
        }
    }

    /// Create a sync manager with a custom sync directory (for testing).
    pub fn with_sync_dir(git: G, sync_dir: PathBuf) -> Self {
        Self { git, sync_dir }
    }

    /// Check if sync is initialized (git repository exists).
    pub fn is_initialized(&self) -> bool {
        self.sync_dir.join(".git").exists()
    }

    /// Initialize sync repository.
    ///
    /// Creates the sync directory, initializes git, and optionally adds a remote.
    pub async fn init(&self, remote_url: Option<String>) -> Result<(), SyncError> {
        // Create sync directory if it doesn't exist
        if !self.sync_dir.exists() {
            std::fs::create_dir_all(&self.sync_dir)?;
        }

        // Initialize git repository
        self.git.init(&self.sync_dir)?;

        // Add remote if provided
        if let Some(url) = remote_url {
            self.git.add_remote(&self.sync_dir, "origin", &url)?;
        }

        Ok(())
    }

    /// Export database to JSONL and push to remote.
    pub async fn export<D: Database>(
        &self,
        db: &D,
        message: Option<String>,
    ) -> Result<ExportSummary, SyncError> {
        if !self.is_initialized() {
            return Err(SyncError::NotInitialized);
        }

        // Pull latest changes first
        if self.has_remote()? {
            // Only pull if remote exists, ignore errors (might be first push)
            let _ = self.git.pull(&self.sync_dir, "origin", "main");
        }

        // Export to JSONL
        let summary = export_all(db, &self.sync_dir).await?;

        // Add all JSONL files
        let files = vec![
            "repos.jsonl".to_string(),
            "projects.jsonl".to_string(),
            "lists.jsonl".to_string(),
            "tasks.jsonl".to_string(),
            "notes.jsonl".to_string(),
        ];
        self.git.add_files(&self.sync_dir, &files)?;

        // Commit
        let commit_msg = message.unwrap_or_else(|| format!("Export {} entities", summary.total()));
        self.git.commit(&self.sync_dir, &commit_msg)?;

        // Push if remote exists
        if self.has_remote()? {
            self.git.push(&self.sync_dir, "origin", "main")?;
        }

        Ok(summary)
    }

    /// Pull from remote and import JSONL to database.
    pub async fn import<D: Database>(&self, db: &D) -> Result<ImportSummary, SyncError> {
        if !self.is_initialized() {
            return Err(SyncError::NotInitialized);
        }

        // Pull latest changes
        if self.has_remote()? {
            self.git.pull(&self.sync_dir, "origin", "main")?;
        }

        // Import from JSONL
        let summary = import_all(db, &self.sync_dir).await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SqliteDatabase;
    use crate::sync::git::MockGitOps;
    use mockall::predicate::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};
    use tempfile::TempDir;

    fn mock_output(code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    async fn setup_test_db() -> SqliteDatabase {
        let db = SqliteDatabase::in_memory().await.unwrap();
        db.migrate().unwrap();
        db
    }

    #[test]
    fn test_is_initialized_false() {
        let temp_dir = TempDir::new().unwrap();
        let mock_git = MockGitOps::new();
        let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

        assert!(!manager.is_initialized());
    }

    #[test]
    fn test_is_initialized_true() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();

        let mock_git = MockGitOps::new();
        let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

        assert!(manager.is_initialized());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_creates_directory_and_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let sync_dir = temp_dir.path().join("sync");

        let mut mock_git = MockGitOps::new();
        mock_git
            .expect_init()
            .with(eq(sync_dir.clone()))
            .times(1)
            .returning(|_| Ok(mock_output(0, "Initialized", "")));

        let manager = SyncManager::with_sync_dir(mock_git, sync_dir.clone());
        manager.init(None).await.unwrap();

        assert!(sync_dir.exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_with_remote() {
        let temp_dir = TempDir::new().unwrap();
        let sync_dir = temp_dir.path().to_path_buf();

        let mut mock_git = MockGitOps::new();
        mock_git
            .expect_init()
            .times(1)
            .returning(|_| Ok(mock_output(0, "Initialized", "")));
        mock_git
            .expect_add_remote()
            .with(
                eq(sync_dir.clone()),
                eq("origin"),
                eq("https://github.com/test/repo.git"),
            )
            .times(1)
            .returning(|_, _, _| Ok(mock_output(0, "", "")));

        let manager = SyncManager::with_sync_dir(mock_git, sync_dir);
        manager
            .init(Some("https://github.com/test/repo.git".to_string()))
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_status_not_initialized() {
        let temp_dir = TempDir::new().unwrap();
        let db = setup_test_db().await;

        let mock_git = MockGitOps::new();
        let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

        let status = manager.status(&db).await.unwrap();

        assert!(!status.initialized);
        assert!(status.remote_url.is_none());
        assert!(status.git_status.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_export_not_initialized() {
        let temp_dir = TempDir::new().unwrap();
        let db = setup_test_db().await;

        let mock_git = MockGitOps::new();
        let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

        let result = manager.export(&db, None).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SyncError::NotInitialized));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_status_initialized_clean() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
        let db = setup_test_db().await;

        let mut mock_git = MockGitOps::new();
        mock_git
            .expect_remote_get_url()
            .returning(|_, _| Ok(mock_output(0, "https://github.com/test/repo.git\n", "")));
        mock_git
            .expect_status_porcelain()
            .returning(|_| Ok(mock_output(0, "", "")));

        let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
        let status = manager.status(&db).await.unwrap();

        assert!(status.initialized);
        assert_eq!(
            status.remote_url,
            Some("https://github.com/test/repo.git".to_string())
        );
        assert!(status.git_status.as_ref().unwrap().clean);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_status_initialized_dirty() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
        let db = setup_test_db().await;

        let mut mock_git = MockGitOps::new();
        mock_git
            .expect_remote_get_url()
            .returning(|_, _| Err(GitError::GitNotFound)); // No remote
        mock_git
            .expect_status_porcelain()
            .returning(|_| Ok(mock_output(0, " M repos.jsonl\n", "")));

        let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
        let status = manager.status(&db).await.unwrap();

        assert!(status.initialized);
        assert!(status.remote_url.is_none());
        assert!(!status.git_status.as_ref().unwrap().clean);
    }
}
