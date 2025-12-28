//! Application state for the API server.

use std::sync::Arc;

use crate::db::Database;
use crate::sync::{GitOps, SyncManager};

/// Shared application state.
///
/// Contains the database connection, sync manager, and is shared across all handlers.
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
}

// Manual Clone impl - we only need Arc to be cloneable, not D or G
// We wrap sync_manager in Arc to make it cloneable
impl<D: Database, G: GitOps + Send + Sync> Clone for AppState<D, G> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            sync_manager: self.sync_manager.clone(), // SyncManager::clone() clones the Arc, not the G
        }
    }
}

impl<D: Database, G: GitOps + Send + Sync> AppState<D, G> {
    /// Create a new AppState with the given database and sync manager.
    ///
    /// # SOLID: Dependency Inversion Principle
    ///
    /// Both database and sync_manager are injected dependencies.
    pub fn new(db: D, sync_manager: SyncManager<G>) -> Self {
        Self {
            db: Arc::new(db),
            sync_manager,
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
}
