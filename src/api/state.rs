//! Application state for the API server.

use std::path::PathBuf;
use std::sync::Arc;

use super::notifier::ChangeNotifier;
use crate::db::Database;
use crate::jobs::{JobExecutor, JobQueue};
use crate::sync::{GitOps, SyncManager};

/// Shared application state.
///
/// Contains the database connection, sync manager, change notifier, and job queue for background tasks.
///
/// # SOLID Principles
///
/// Follows Dependency Inversion Principle (DIP):
/// - Generic over `D: Database` - Can use any database implementation
/// - Generic over `G: GitOps` - Can use RealGit (production) or MockGitOps (tests)
///
/// Dependencies are injected via constructor, not created internally.
pub struct AppState<D: Database, G: GitOps + Send + Sync> {
    db: Arc<D>,
    sync_manager: SyncManager<G>,
    notifier: ChangeNotifier,
    skills_dir: PathBuf,
    job_queue: JobQueue,
    job_executor: JobExecutor,
}

// Manual Clone impl - we only need Arc to be cloneable, not D or G
// JobQueue and JobExecutor are Clone (use Arc internally)
// ChangeNotifier is Clone (derives it)
impl<D: Database, G: GitOps + Send + Sync> Clone for AppState<D, G> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            sync_manager: self.sync_manager.clone(), // SyncManager::clone() clones the Arc, not the G
            notifier: self.notifier.clone(),
            skills_dir: self.skills_dir.clone(),
            job_queue: self.job_queue.clone(),
            job_executor: self.job_executor.clone(),
        }
    }
}

impl<D: Database, G: GitOps + Send + Sync> AppState<D, G> {
    /// Create a new AppState with the given database, sync manager, notifier, and job infrastructure.
    ///
    /// # SOLID: Dependency Inversion Principle
    ///
    /// All dependencies are injected via constructor:
    /// - `db`: Database implementation
    /// - `sync_manager`: Sync manager with GitOps implementation
    /// - `notifier`: Change notifier for pub/sub
    /// - `skills_dir`: Skills cache directory path
    /// - `job_queue`: Job queue for background tasks
    /// - `job_executor`: Job executor for running jobs
    pub fn new(
        db: D,
        sync_manager: SyncManager<G>,
        notifier: ChangeNotifier,
        skills_dir: PathBuf,
        job_queue: JobQueue,
        job_executor: JobExecutor,
    ) -> Self {
        Self {
            db: Arc::new(db),
            sync_manager,
            notifier,
            skills_dir,
            job_queue,
            job_executor,
        }
    }

    /// Get a reference to the database.
    pub fn db(&self) -> &D {
        &self.db
    }

    /// Get a cloned Arc to the database.
    ///
    /// Useful for passing the database to services that need Arc<D>.
    pub fn db_arc(&self) -> Arc<D> {
        Arc::clone(&self.db)
    }

    /// Get a reference to the sync manager.
    pub fn sync_manager(&self) -> &SyncManager<G> {
        &self.sync_manager
    }

    /// Get a reference to the change notifier.
    pub fn notifier(&self) -> &ChangeNotifier {
        &self.notifier
    }

    /// Get a reference to the skills directory path.
    pub fn skills_dir(&self) -> &PathBuf {
        &self.skills_dir
    }

    /// Get a reference to the job queue.
    pub fn job_queue(&self) -> &JobQueue {
        &self.job_queue
    }

    /// Get a reference to the job executor.
    pub fn job_executor(&self) -> &JobExecutor {
        &self.job_executor
    }
}
