//! Application state for the API server.

use std::path::PathBuf;
use std::sync::Arc;

use super::notifier::ChangeNotifier;
use crate::db::Database;
use crate::sync::{GitOps, SyncManager};

/// Shared application state.
///
/// Contains the database connection, sync manager, and change notifier.
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

impl<D: Database, G: GitOps + Send + Sync> Clone for AppState<D, G> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            sync_manager: self.sync_manager.clone(),
            notifier: self.notifier.clone(),
            skills_dir: self.skills_dir.clone(),
        }
    }
}

impl<D: Database, G: GitOps + Send + Sync> AppState<D, G> {
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

    pub fn db(&self) -> &D {
        &self.db
    }

    pub fn db_arc(&self) -> Arc<D> {
        Arc::clone(&self.db)
    }

    pub fn sync_manager(&self) -> &SyncManager<G> {
        &self.sync_manager
    }

    pub fn notifier(&self) -> &ChangeNotifier {
        &self.notifier
    }

    pub fn skills_dir(&self) -> &PathBuf {
        &self.skills_dir
    }
}
