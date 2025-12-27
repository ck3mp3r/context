//! Application state for the API server.

use std::sync::Arc;

use crate::db::Database;

/// Shared application state.
///
/// Contains the database connection and is shared across all handlers.
/// Generic over any Database implementation.
pub struct AppState<D: Database> {
    db: Arc<D>,
}

// Manual Clone impl - we only need Arc to be cloneable, not D
impl<D: Database> Clone for AppState<D> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
        }
    }
}

impl<D: Database> AppState<D> {
    /// Create a new AppState with the given database.
    pub fn new(db: D) -> Self {
        Self { db: Arc::new(db) }
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
}
