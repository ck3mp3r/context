//! Application state for the API server.

use std::path::PathBuf;
use std::sync::Arc;

use super::notifier::ChangeNotifier;
use crate::db::Database;
use crate::sync::{GitOps, SyncManager};

/// Shared application state.
///
/// Contains the database connection, sync manager, and change notifier for real-time updates.
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
}

// Manual Clone impl - we only need Arc to be cloneable, not D or G
// We wrap sync_manager in Arc to make it cloneable
// ChangeNotifier is Clone (derives it)
impl<D: Database, G: GitOps + Send + Sync> Clone for AppState<D, G> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            sync_manager: self.sync_manager.clone(), // SyncManager::clone() clones the Arc, not the G
            notifier: self.notifier.clone(),
            skills_dir: self.skills_dir.clone(),
        }
    }
}

impl<D: Database, G: GitOps + Send + Sync> AppState<D, G> {
    /// Create a new AppState with the given database, sync manager, and notifier.
    ///
    /// # SOLID: Dependency Inversion Principle
    ///
    /// All dependencies are injected via constructor:
    /// - `db`: Database implementation
    /// - `sync_manager`: Sync manager with GitOps implementation
    /// - `notifier`: Change notifier for pub/sub
    /// - `skills_dir`: Skills cache directory path
    pub fn new(
        db: D,
        sync_manager: SyncManager<G>,
        notifier: ChangeNotifier,
        skills_dir: PathBuf,
    ) -> Self {
        Self {
            db: Arc::new(db),
            sync_manager,
            notifier,
            skills_dir,
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
}
