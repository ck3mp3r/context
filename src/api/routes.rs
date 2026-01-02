//! API route configuration.

use axum::Router;
use axum::routing::{any, delete, get, patch, post, put};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use super::handlers::{self, HealthResponse};
use super::state::AppState;
use super::static_assets::serve_frontend;
use super::v1::{
    CreateNoteRequest, CreateProjectRequest, CreateRepoRequest, CreateTaskListRequest,
    CreateTaskRequest, ErrorResponse, NoteResponse, PatchNoteRequest, PatchProjectRequest,
    PatchRepoRequest, PatchTaskListRequest, PatchTaskRequest, ProjectResponse, RepoResponse,
    TaskListResponse, TaskResponse, UpdateNoteRequest, UpdateProjectRequest, UpdateRepoRequest,
    UpdateTaskListRequest, UpdateTaskRequest,
};
use crate::db::Database;

/// Build routes with generic database and git types.
///
/// This macro reduces boilerplate when registering handlers that are generic
/// over the Database and GitOps traits. It applies the turbofish operator automatically.
macro_rules! routes {
    ($D:ty, $G:ty => {
        $($method:ident $path:literal => $($handler:ident)::+),* $(,)?
    }) => {{
        let router = Router::new();
        $(
            let router = router.route($path, $method($($handler)::+::<$D, $G>));
        )*
        router
    }};
}

/// OpenAPI documentation
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Context API",
        version = "0.1.0",
        description = "Context/memory management API for LLM sessions",
        license(name = "MIT")
    ),
    paths(
        handlers::health,
        super::v1::list_projects,
        super::v1::get_project,
        super::v1::create_project,
        super::v1::update_project,
        super::v1::patch_project,
        super::v1::delete_project,
        super::v1::list_repos,
        super::v1::get_repo,
        super::v1::create_repo,
        super::v1::update_repo,
        super::v1::patch_repo,
        super::v1::delete_repo,
        super::v1::list_task_lists,
        super::v1::get_task_list,
        super::v1::create_task_list,
        super::v1::update_task_list,
        super::v1::patch_task_list,
        super::v1::delete_task_list,
        super::v1::get_task_list_stats,
        super::v1::list_tasks,
        super::v1::get_task,
        super::v1::create_task,
        super::v1::update_task,
        super::v1::patch_task,
        super::v1::delete_task,
        super::v1::list_notes,
        super::v1::get_note,
        super::v1::create_note,
        super::v1::update_note,
        super::v1::patch_note,
        super::v1::delete_note,
        super::v1::init_sync,
        super::v1::export_sync,
        super::v1::import_sync,
        super::v1::get_sync_status,
    ),
    components(
        schemas(
            HealthResponse,
            ProjectResponse,
            CreateProjectRequest,
            UpdateProjectRequest,
            PatchProjectRequest,
            super::v1::PaginatedProjects,
            RepoResponse,
            CreateRepoRequest,
            UpdateRepoRequest,
            PatchRepoRequest,
            super::v1::PaginatedRepos,
            TaskListResponse,
            CreateTaskListRequest,
            UpdateTaskListRequest,
            PatchTaskListRequest,
            super::v1::PaginatedTaskLists,
            super::v1::TaskStatsResponse,
            TaskResponse,
            CreateTaskRequest,
            UpdateTaskRequest,
            PatchTaskRequest,
            super::v1::PaginatedTasks,
            NoteResponse,
            CreateNoteRequest,
            UpdateNoteRequest,
            PatchNoteRequest,
            super::v1::PaginatedNotes,
            super::v1::InitSyncRequest,
            super::v1::ExportSyncRequest,
            super::v1::SyncResponse,
            ErrorResponse,
        )
    ),
    tags(
        (name = "system", description = "System health and status endpoints"),
        (name = "projects", description = "Project management endpoints"),
        (name = "repos", description = "Repository management endpoints"),
        (name = "task-lists", description = "Task list management endpoints"),
        (name = "tasks", description = "Task management endpoints"),
        (name = "notes", description = "Note management endpoints with FTS search"),
        (name = "sync", description = "Git-based sync operations")
    )
)]
pub struct ApiDoc;

/// Create the API router with OpenAPI documentation and MCP server
pub fn create_router<D: Database + 'static, G: crate::sync::GitOps + Send + Sync + 'static>(
    state: AppState<D, G>,
    enable_docs: bool,
) -> Router {
    // Create MCP service (Model Context Protocol server)
    // Uses the same database as the REST API for consistency
    let ct = tokio_util::sync::CancellationToken::new();
    let mcp_service: rmcp::transport::streamable_http_server::StreamableHttpService<
        crate::mcp::McpServer<D>,
    > = crate::mcp::create_mcp_service(state.db_arc(), ct);

    // System routes (non-generic, not versioned)
    let system_routes = Router::new()
        .route("/health", get(handlers::health))
        .route("/ws", any(super::websocket::ws_handler::<D, G>));

    // V1 API routes (generic over Database and GitOps)
    let v1_routes = routes!(D, G => {
        // Projects
        get "/projects" => super::v1::list_projects,
        get "/projects/{id}" => super::v1::get_project,
        post "/projects" => super::v1::create_project,
        put "/projects/{id}" => super::v1::update_project,
        patch "/projects/{id}" => super::v1::patch_project,
        delete "/projects/{id}" => super::v1::delete_project,
        // Repos
        get "/repos" => super::v1::list_repos,
        get "/repos/{id}" => super::v1::get_repo,
        post "/repos" => super::v1::create_repo,
        put "/repos/{id}" => super::v1::update_repo,
        patch "/repos/{id}" => super::v1::patch_repo,
        delete "/repos/{id}" => super::v1::delete_repo,
        // TaskLists
        get "/task-lists" => super::v1::list_task_lists,
        get "/task-lists/{id}" => super::v1::get_task_list,
        post "/task-lists" => super::v1::create_task_list,
        put "/task-lists/{id}" => super::v1::update_task_list,
        patch "/task-lists/{id}" => super::v1::patch_task_list,
        delete "/task-lists/{id}" => super::v1::delete_task_list,
        // Tasks
        get "/task-lists/{list_id}/tasks" => super::v1::list_tasks,
        post "/task-lists/{list_id}/tasks" => super::v1::create_task,
        get "/tasks/{id}" => super::v1::get_task,
        put "/tasks/{id}" => super::v1::update_task,
        patch "/tasks/{id}" => super::v1::patch_task,
        delete "/tasks/{id}" => super::v1::delete_task,
        // Notes
        get "/notes" => super::v1::list_notes,
        get "/notes/{id}" => super::v1::get_note,
        post "/notes" => super::v1::create_note,
        put "/notes/{id}" => super::v1::update_note,
        patch "/notes/{id}" => super::v1::patch_note,
        delete "/notes/{id}" => super::v1::delete_note,
        // Sync
        post "/sync/init" => super::v1::init_sync,
        post "/sync/export" => super::v1::export_sync,
        post "/sync/import" => super::v1::import_sync,
        get "/sync/status" => super::v1::get_sync_status,
        get "/task-lists/{id}/stats" => super::v1::get_task_list_stats,
    });

    let mut router = system_routes
        .nest("/api/v1", v1_routes)
        .nest_service("/mcp", mcp_service); // MCP server endpoint

    // Conditionally add OpenAPI docs endpoint
    if enable_docs {
        let api = ApiDoc::openapi();
        router = router.merge(Scalar::with_url("/docs", api));
    }

    router.with_state(state).fallback(serve_frontend) // Serve embedded frontend assets for all unmatched routes
}
