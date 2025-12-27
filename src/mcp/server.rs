//! MCP server implementation
//!
//! This module implements the main MCP server coordinator that manages
//! all tool handlers following SOLID principles.

use std::sync::Arc;

use rmcp::{
    ServerHandler,
    model::{ServerCapabilities, ServerInfo},
};

use crate::db::Database;

use super::tools::{NoteTools, ProjectTools, RepoTools, TaskListTools, TaskTools};

/// Main MCP server coordinator
///
/// Generic over `D: Database` for zero-cost abstraction (no dynamic dispatch).
///
/// # SOLID Principles
///
/// - **Single Responsibility**: Coordinates tool handlers
/// - **Open/Closed**: New tools can be added without modifying this struct
/// - **Dependency Inversion**: Depends on Database trait, not concrete implementation
///
/// # Architecture
///
/// The server delegates to separate tool structs, each responsible for one entity type:
/// - ProjectTools: Project operations
/// - RepoTools: Repository operations
/// - TaskListTools: Task list operations
/// - TaskTools: Task operations
/// - NoteTools: Note operations
#[derive(Clone)]
pub struct McpServer<D: Database> {
    db: Arc<D>,
    _project_tools: ProjectTools<D>,
    _repo_tools: RepoTools<D>,
    _task_list_tools: TaskListTools<D>,
    _task_tools: TaskTools<D>,
    _note_tools: NoteTools<D>,
}

impl<D: Database> McpServer<D> {
    /// Create a new MCP server with the given database
    ///
    /// # Arguments
    /// * `db` - Database instance implementing the Database trait
    ///
    /// # Returns
    /// A new McpServer instance with all tool handlers initialized
    pub fn new(db: D) -> Self {
        let db = Arc::new(db);

        Self {
            _project_tools: ProjectTools::new(Arc::clone(&db)),
            _repo_tools: RepoTools::new(Arc::clone(&db)),
            _task_list_tools: TaskListTools::new(Arc::clone(&db)),
            _task_tools: TaskTools::new(Arc::clone(&db)),
            _note_tools: NoteTools::new(Arc::clone(&db)),
            db,
        }
    }
}

impl<D: Database + 'static> ServerHandler for McpServer<D> {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "C5T MCP Server - Manage projects, repositories, task lists, tasks, and notes"
                    .to_string(),
            ),
            ..Default::default()
        }
    }
}
