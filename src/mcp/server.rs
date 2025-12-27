//! MCP server implementation
//!
//! This module implements the main MCP server coordinator that manages
//! all tool handlers following SOLID principles.

use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};

use crate::db::Database;

use super::tools::{
    NoteTools, ProjectTools, RepoTools, TaskListTools, TaskTools, projects::*, repos::*,
    task_lists::*,
};

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
    #[allow(dead_code)] // Will be used when Task/Note tools are implemented
    db: Arc<D>,
    project_tools: ProjectTools<D>,
    repo_tools: RepoTools<D>,
    task_list_tools: TaskListTools<D>,
    #[allow(dead_code)] // Will be used in Phase 4
    task_tools: TaskTools<D>,
    #[allow(dead_code)] // Will be used in Phase 5
    note_tools: NoteTools<D>,
    #[allow(dead_code)] // Used by #[tool_router] macro
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> McpServer<D> {
    /// Create a new MCP server with the given database
    ///
    /// # Arguments
    /// * `db` - Database instance (can be Arc<D> or D)
    ///
    /// # Returns
    /// A new McpServer instance with all tool handlers initialized
    pub fn new(db: impl Into<Arc<D>>) -> Self {
        let db = db.into();

        Self {
            project_tools: ProjectTools::new(Arc::clone(&db)),
            repo_tools: RepoTools::new(Arc::clone(&db)),
            task_list_tools: TaskListTools::new(Arc::clone(&db)),
            task_tools: TaskTools::new(Arc::clone(&db)),
            note_tools: NoteTools::new(Arc::clone(&db)),
            db,
            tool_router: Self::tool_router(),
        }
    }

    // =========================================================================
    // Project Tools
    // =========================================================================

    #[tool(description = "List all projects")]
    pub async fn list_projects(&self) -> Result<CallToolResult, McpError> {
        self.project_tools.list_projects().await
    }

    #[tool(description = "Get a project by ID")]
    pub async fn get_project(
        &self,
        params: Parameters<GetProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.get_project(params).await
    }

    #[tool(description = "Create a new project")]
    pub async fn create_project(
        &self,
        params: Parameters<CreateProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.create_project(params).await
    }

    #[tool(description = "Update an existing project")]
    pub async fn update_project(
        &self,
        params: Parameters<UpdateProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.update_project(params).await
    }

    #[tool(description = "Delete a project")]
    pub async fn delete_project(
        &self,
        params: Parameters<DeleteProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.delete_project(params).await
    }

    // =========================================================================
    // Repository Tools
    // =========================================================================

    #[tool(description = "List all repositories")]
    pub async fn list_repos(&self) -> Result<CallToolResult, McpError> {
        self.repo_tools.list_repos().await
    }

    #[tool(description = "Get a repository by ID")]
    pub async fn get_repo(
        &self,
        params: Parameters<GetRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.get_repo(params).await
    }

    #[tool(description = "Create a new repository")]
    pub async fn create_repo(
        &self,
        params: Parameters<CreateRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.create_repo(params).await
    }

    #[tool(description = "Update an existing repository")]
    pub async fn update_repo(
        &self,
        params: Parameters<UpdateRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.update_repo(params).await
    }

    #[tool(description = "Delete a repository")]
    pub async fn delete_repo(
        &self,
        params: Parameters<DeleteRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.delete_repo(params).await
    }

    // =========================================================================
    // TaskList Tools
    // =========================================================================

    #[tool(description = "List all task lists with optional filtering")]
    pub async fn list_task_lists(
        &self,
        params: Parameters<ListTaskListsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.list_task_lists(params).await
    }

    #[tool(description = "Get a task list by ID")]
    pub async fn get_task_list(
        &self,
        params: Parameters<GetTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.get_task_list(params).await
    }

    #[tool(description = "Create a new task list")]
    pub async fn create_task_list(
        &self,
        params: Parameters<CreateTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.create_task_list(params).await
    }

    #[tool(description = "Update an existing task list")]
    pub async fn update_task_list(
        &self,
        params: Parameters<UpdateTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.update_task_list(params).await
    }

    #[tool(description = "Delete a task list")]
    pub async fn delete_task_list(
        &self,
        params: Parameters<DeleteTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.delete_task_list(params).await
    }
}

#[tool_handler]
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
