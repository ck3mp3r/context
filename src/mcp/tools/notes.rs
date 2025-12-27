//! Note tool implementations
//!
//! Handles all MCP tools for note management operations.
//! Follows Single Responsibility Principle (SRP).

use crate::db::Database;
use rmcp::{handler::server::router::tool::ToolRouter, tool_router};
use std::sync::Arc;

/// Note management tools
///
/// Generic over `D: Database` for zero-cost abstraction.
///
/// # SOLID Principles
/// - **Single Responsibility**: Only handles note operations
/// - **Dependency Inversion**: Depends on Database trait
#[derive(Clone)]
pub struct NoteTools<D: Database> {
    #[allow(dead_code)]
    db: Arc<D>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> NoteTools<D> {
    /// Create new NoteTools with database
    pub fn new(db: Arc<D>) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    /// Get the tool router for this handler
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }
}
