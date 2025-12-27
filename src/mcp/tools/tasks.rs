//! Task tool implementations
//!
//! Handles all MCP tools for task management operations.
//! Follows Single Responsibility Principle (SRP).

use crate::db::Database;
use std::sync::Arc;

/// Task management tools
///
/// Generic over `D: Database` for zero-cost abstraction.
///
/// # SOLID Principles
/// - **Single Responsibility**: Only handles task operations
/// - **Dependency Inversion**: Depends on Database trait
#[derive(Clone)]
pub struct TaskTools<D: Database> {
    db: Arc<D>,
}

impl<D: Database> TaskTools<D> {
    /// Create new TaskTools with database
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }
}
