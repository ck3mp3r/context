//! MCP server implementation
//!
//! This module implements the main MCP server coordinator that manages
//! all tool handlers following SOLID principles.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ProtocolVersion, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};

use crate::api::notifier::ChangeNotifier;
use context_core::Database;
use context_sync::RealGit;

use super::tools::{
    NoteTools, ProjectTools, RepoTools, SkillTools, SyncTools, TaskListTools, TaskTools, notes::*,
    projects::*, repos::*, skills::*, sync::*, task_lists::*, tasks::*,
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
/// - SkillTools: Skill operations
pub struct McpServer<D: Database> {
    project_tools: ProjectTools<D>,
    repo_tools: RepoTools<D>,
    task_list_tools: TaskListTools<D>,
    task_tools: TaskTools<D>,
    note_tools: NoteTools<D>,
    skill_tools: SkillTools<D>,
    sync_tools: SyncTools<D, RealGit>,
    #[allow(dead_code)] // Used by #[tool_router] macro
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> McpServer<D> {
    /// Create a new MCP server with the given database and notifier
    ///
    /// # Arguments
    /// * `db` - Database instance (can be `Arc<D>` or `D`)
    /// * `notifier` - ChangeNotifier for broadcasting updates
    /// * `skills_dir` - Path to skills cache directory
    ///
    /// # Returns
    /// A new McpServer instance with all tool handlers initialized
    pub fn new(db: impl Into<Arc<D>>, notifier: ChangeNotifier, skills_dir: PathBuf) -> Self {
        let db = db.into();

        Self {
            project_tools: ProjectTools::new(Arc::clone(&db), notifier.clone()),
            repo_tools: RepoTools::new(Arc::clone(&db), notifier.clone()),
            task_list_tools: TaskListTools::new(Arc::clone(&db), notifier.clone()),
            task_tools: TaskTools::new(Arc::clone(&db), notifier.clone()),
            note_tools: NoteTools::new(Arc::clone(&db), notifier.clone()),
            skill_tools: SkillTools::new(Arc::clone(&db), notifier.clone(), skills_dir),
            sync_tools: SyncTools::with_real_git(Arc::clone(&db)),
            tool_router: Self::tool_router(),
        }
    }

    // =========================================================================
    // Project Tools
    // =========================================================================

    #[tool(description = "List projects with pagination (default: 10, max: 20)")]
    pub async fn projects_list(
        &self,
        params: Parameters<ListProjectsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.projects_list(params).await
    }

    #[tool(description = "Get a project by ID")]
    pub async fn project_get(
        &self,
        params: Parameters<GetProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.project_get(params).await
    }

    #[tool(description = "Create a new project")]
    pub async fn project_create(
        &self,
        params: Parameters<CreateProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.project_create(params).await
    }

    #[tool(description = "Update an existing project")]
    pub async fn project_update(
        &self,
        params: Parameters<UpdateProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.project_update(params).await
    }

    #[tool(description = "Delete a project")]
    pub async fn project_delete(
        &self,
        params: Parameters<DeleteProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.project_delete(params).await
    }

    #[tool(
        description = "Restore a soft-deleted project (cascades to children deleted in the same delete)"
    )]
    pub async fn project_restore(
        &self,
        params: Parameters<RestoreProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.project_tools.project_restore(params).await
    }

    // =========================================================================
    // Repository Tools
    // =========================================================================

    #[tool(description = "List repositories with pagination (default: 10, max: 20)")]
    pub async fn repos_list(
        &self,
        params: Parameters<ListReposParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.repos_list(params).await
    }

    #[tool(description = "Get a repository by ID")]
    pub async fn repo_get(
        &self,
        params: Parameters<GetRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.repo_get(params).await
    }

    #[tool(description = "Create a new repository")]
    pub async fn repo_create(
        &self,
        params: Parameters<CreateRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.repo_create(params).await
    }

    #[tool(description = "Update an existing repository")]
    pub async fn repo_update(
        &self,
        params: Parameters<UpdateRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.repo_update(params).await
    }

    #[tool(description = "Delete a repository")]
    pub async fn repo_delete(
        &self,
        params: Parameters<DeleteRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.repo_delete(params).await
    }

    #[tool(description = "Restore a soft-deleted repository")]
    pub async fn repo_restore(
        &self,
        params: Parameters<RestoreRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        self.repo_tools.repo_restore(params).await
    }

    // =========================================================================
    // TaskList Tools
    // =========================================================================

    #[tool(description = "List all task lists with optional filtering")]
    pub async fn task_lists_list(
        &self,
        params: Parameters<ListTaskListsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.task_lists_list(params).await
    }

    #[tool(description = "Get a task list by ID")]
    pub async fn task_list_get(
        &self,
        params: Parameters<GetTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.task_list_get(params).await
    }

    #[tool(description = "Create a new task list")]
    pub async fn task_list_create(
        &self,
        params: Parameters<CreateTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.task_list_create(params).await
    }

    #[tool(description = "Update an existing task list")]
    pub async fn task_list_update(
        &self,
        params: Parameters<UpdateTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.task_list_update(params).await
    }

    #[tool(description = "Delete a task list")]
    pub async fn task_list_delete(
        &self,
        params: Parameters<DeleteTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.task_list_delete(params).await
    }

    #[tool(
        description = "Restore a soft-deleted task list (cascades to tasks deleted in the same delete). Fails when the parent project is still deleted."
    )]
    pub async fn task_list_restore(
        &self,
        params: Parameters<RestoreTaskListParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.task_list_restore(params).await
    }

    #[tool(description = "Get task statistics for a task list")]
    pub async fn task_list_stats_get(
        &self,
        params: Parameters<GetTaskListStatsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_list_tools.task_list_stats_get(params).await
    }

    // =========================================================================
    // Task Tools
    // =========================================================================

    #[tool(description = "List tasks for a task list with optional filtering")]
    pub async fn tasks_list(
        &self,
        params: Parameters<ListTasksParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.tasks_list(params).await
    }

    #[tool(description = "Get a task by ID")]
    pub async fn task_get(
        &self,
        params: Parameters<GetTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.task_get(params).await
    }

    #[tool(description = "Create a new task")]
    pub async fn task_create(
        &self,
        params: Parameters<CreateTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.task_create(params).await
    }

    #[tool(description = "Update an existing task")]
    pub async fn task_update(
        &self,
        params: Parameters<UpdateTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.task_update(params).await
    }

    #[tool(
        description = "Transition task between statuses. Cascades to subtasks with matching status. Transitions: backlog→[todo,in_progress,cancelled], todo→[backlog,in_progress,cancelled], in_progress→[todo,review,done,cancelled], review→[in_progress,done,cancelled], done/cancelled→[backlog,todo,in_progress,review]."
    )]
    pub async fn task_transition(
        &self,
        params: Parameters<TransitionTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.task_transition(params).await
    }

    #[tool(description = "Delete a task")]
    pub async fn task_delete(
        &self,
        params: Parameters<DeleteTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.task_delete(params).await
    }

    #[tool(
        description = "Restore a soft-deleted task (cascades to descendants deleted in the same delete). Fails when the parent task list is still deleted."
    )]
    pub async fn task_restore(
        &self,
        params: Parameters<RestoreTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        self.task_tools.task_restore(params).await
    }

    // =========================================================================
    // Note Tools
    // =========================================================================

    #[tool(description = "List notes with optional filtering")]
    pub async fn notes_list(
        &self,
        params: Parameters<ListNotesParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.notes_list(params).await
    }

    #[tool(description = "Create a new note")]
    pub async fn note_create(
        &self,
        params: Parameters<CreateNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.note_create(params).await
    }

    #[tool(description = "Delete a note")]
    pub async fn note_delete(
        &self,
        params: Parameters<DeleteNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.note_delete(params).await
    }

    #[tool(description = "Restore a soft-deleted note")]
    pub async fn note_restore(
        &self,
        params: Parameters<RestoreNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.note_restore(params).await
    }

    #[tool(
        description = "Read a note. Omit ranges for full content, use empty array [] for metadata only, or specify ranges for specific lines."
    )]
    pub async fn note_read(
        &self,
        params: Parameters<ReadNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.note_read(params).await
    }

    #[tool(
        description = "Edit a note - update metadata (title, tags, relationships) and/or apply line-range patches to content. All fields optional. Patches are automatically sorted, validated, and applied in reverse order to maintain accurate line numbers."
    )]
    pub async fn note_edit(
        &self,
        params: Parameters<EditNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.note_tools.note_edit(params).await
    }

    // =========================================================================
    // Skill Tools
    // =========================================================================

    #[tool(description = "List skills with optional filtering")]
    pub async fn skills_list(
        &self,
        params: Parameters<ListSkillsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.skill_tools.skills_list(params).await
    }

    #[tool(description = "Get a skill by ID")]
    pub async fn skill_get(
        &self,
        params: Parameters<GetSkillParams>,
    ) -> Result<CallToolResult, McpError> {
        self.skill_tools.skill_get(params).await
    }

    #[tool(description = "Update skill tags and/or project_ids (partial updates)")]
    pub async fn skill_update(
        &self,
        params: Parameters<UpdateSkillParams>,
    ) -> Result<CallToolResult, McpError> {
        self.skill_tools.skill_update(params).await
    }

    #[tool(description = "Restore a soft-deleted skill")]
    pub async fn skill_restore(
        &self,
        params: Parameters<RestoreSkillParams>,
    ) -> Result<CallToolResult, McpError> {
        self.skill_tools.skill_restore(params).await
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
    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::default();
        info.protocol_version = ProtocolVersion::LATEST;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::from_build_env()
            .with_title("C5T MCP Server")
            .with_description(
                "Manage projects, repositories, task lists, tasks, notes, and skills",
            );
        info.instructions = Some(
            "C5T MCP Server - Manage projects, repositories, task lists, tasks, notes, and skills"
                .to_string(),
        );
        info
    }
}
