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
use crate::sync::RealGit;

use super::tools::{
    NoteTools, ProjectTools, RepoTools, SyncTools, TaskListTools, TaskTools, notes::*, projects::*,
    repos::*, sync::*, task_lists::*, tasks::*,
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
    task_tools: TaskTools<D>,
    note_tools: NoteTools<D>,
    sync_tools: SyncTools<D, RealGit>,
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
            sync_tools: SyncTools::with_real_git(Arc::clone(&db)),
            db,
            tool_router: Self::tool_router(),
        }
    }

    // =========================================================================
    // Project Tools
    // =========================================================================

    #[tool(description = "List projects with pagination (default: 10, max: 20)")]
    pub async fn list_projects(
        &self,
        params: Parameters<ListProjectsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.list_projects(params).await
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

    #[tool(description = "List repositories with pagination (default: 10, max: 20)")]
    pub async fn list_repos(
        &self,
        params: Parameters<ListReposParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.list_repos(params).await
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

    #[tool(description = "Get task statistics for a task list")]
    pub async fn get_task_list_stats(
        &self,
        params: Parameters<GetTaskListStatsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.get_task_list_stats(params).await
    }

    // =========================================================================
    // Task Tools
    // =========================================================================

    #[tool(description = "List tasks for a task list with optional filtering")]
    pub async fn list_tasks(
        &self,
        params: Parameters<ListTasksParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.list_tasks(params).await
    }

    #[tool(description = "Get a task by ID")]
    pub async fn get_task(
        &self,
        params: Parameters<GetTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.get_task(params).await
    }

    #[tool(description = "Create a new task")]
    pub async fn create_task(
        &self,
        params: Parameters<CreateTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.create_task(params).await
    }

    #[tool(description = "Update an existing task")]
    pub async fn update_task(
        &self,
        params: Parameters<UpdateTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.update_task(params).await
    }

    #[tool(description = "Mark a task as complete")]
    pub async fn complete_task(
        &self,
        params: Parameters<CompleteTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.complete_task(params).await
    }

    #[tool(description = "Delete a task")]
    pub async fn delete_task(
        &self,
        params: Parameters<DeleteTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.delete_task(params).await
    }

    // =========================================================================
    // Note Tools
    // =========================================================================

    #[tool(description = "List notes with optional filtering")]
    pub async fn list_notes(
        &self,
        params: Parameters<ListNotesParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.list_notes(params).await
    }

    #[tool(description = "Get a note by ID")]
    pub async fn get_note(
        &self,
        params: Parameters<GetNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.get_note(params).await
    }

    #[tool(description = "Create a new note")]
    pub async fn create_note(
        &self,
        params: Parameters<CreateNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.create_note(params).await
    }

    #[tool(description = "Update an existing note")]
    pub async fn update_note(
        &self,
        params: Parameters<UpdateNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.update_note(params).await
    }

    #[tool(description = "Delete a note")]
    pub async fn delete_note(
        &self,
        params: Parameters<DeleteNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.delete_note(params).await
    }

    #[tool(description = "Full-text search notes using FTS5")]
    pub async fn search_notes(
        &self,
        params: Parameters<SearchNotesParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.search_notes(params).await
    }

    // =========================================================================
    // Sync Tools
    // =========================================================================

    #[tool(description = "Git-based sync: init, export, import, or status")]
    pub async fn sync(&self, params: Parameters<SyncParams>) -> Result<CallToolResult, McpError> {
        self.sync_tools.sync(params).await
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
