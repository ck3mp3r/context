//! API route configuration.

use axum::Router;
use axum::routing::{delete, get, patch, post, put};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use super::handlers::{self, HealthResponse};
use super::state::AppState;
use super::v1::{
    CreateNoteRequest, CreateProjectRequest, CreateRepoRequest, CreateTaskListRequest,
    CreateTaskRequest, ErrorResponse, NoteResponse, PatchProjectRequest, ProjectResponse,
    RepoResponse, TaskListResponse, TaskResponse, UpdateNoteRequest, UpdateProjectRequest,
    UpdateRepoRequest, UpdateTaskListRequest, UpdateTaskRequest,
};
use crate::db::Database;

/// Build routes with generic database type.
///
/// This macro reduces boilerplate when registering handlers that are generic
/// over the Database trait. It applies the turbofish operator automatically.
macro_rules! routes {
    ($D:ty => {
        $($method:ident $path:literal => $($handler:ident)::+),* $(,)?
    }) => {{
        let router = Router::new();
        $(
            let router = router.route($path, $method($($handler)::+::<$D>));
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
        handlers::root,
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
        super::v1::delete_repo,
        super::v1::list_task_lists,
        super::v1::get_task_list,
        super::v1::create_task_list,
        super::v1::update_task_list,
        super::v1::delete_task_list,
        super::v1::list_tasks,
        super::v1::get_task,
        super::v1::create_task,
        super::v1::update_task,
        super::v1::delete_task,
        super::v1::list_notes,
        super::v1::get_note,
        super::v1::create_note,
        super::v1::update_note,
        super::v1::delete_note,
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
            super::v1::PaginatedRepos,
            TaskListResponse,
            CreateTaskListRequest,
            UpdateTaskListRequest,
            super::v1::PaginatedTaskLists,
            TaskResponse,
            CreateTaskRequest,
            UpdateTaskRequest,
            super::v1::PaginatedTasks,
            NoteResponse,
            CreateNoteRequest,
            UpdateNoteRequest,
            super::v1::PaginatedNotes,
            ErrorResponse,
        )
    ),
    tags(
        (name = "system", description = "System health and status endpoints"),
        (name = "projects", description = "Project management endpoints"),
        (name = "repos", description = "Repository management endpoints"),
        (name = "task-lists", description = "Task list management endpoints"),
        (name = "tasks", description = "Task management endpoints"),
        (name = "notes", description = "Note management endpoints with FTS search")
    )
)]
pub struct ApiDoc;

/// Create the API router with OpenAPI documentation
pub fn create_router<D: Database + 'static>(state: AppState<D>) -> Router {
    let api = ApiDoc::openapi();

    // System routes (non-generic, not versioned)
    let system_routes = Router::new()
        .route("/", get(handlers::root))
        .route("/health", get(handlers::health));

    // V1 API routes (generic over Database)
    let v1_routes = routes!(D => {
        // Projects
        get "/v1/projects" => super::v1::list_projects,
        get "/v1/projects/{id}" => super::v1::get_project,
        post "/v1/projects" => super::v1::create_project,
        put "/v1/projects/{id}" => super::v1::update_project,
        patch "/v1/projects/{id}" => super::v1::patch_project,
        delete "/v1/projects/{id}" => super::v1::delete_project,
        // Repos
        get "/v1/repos" => super::v1::list_repos,
        get "/v1/repos/{id}" => super::v1::get_repo,
        post "/v1/repos" => super::v1::create_repo,
        put "/v1/repos/{id}" => super::v1::update_repo,
        delete "/v1/repos/{id}" => super::v1::delete_repo,
        // TaskLists
        get "/v1/task-lists" => super::v1::list_task_lists,
        get "/v1/task-lists/{id}" => super::v1::get_task_list,
        post "/v1/task-lists" => super::v1::create_task_list,
        put "/v1/task-lists/{id}" => super::v1::update_task_list,
        delete "/v1/task-lists/{id}" => super::v1::delete_task_list,
        // Tasks
        get "/v1/task-lists/{list_id}/tasks" => super::v1::list_tasks,
        post "/v1/task-lists/{list_id}/tasks" => super::v1::create_task,
        get "/v1/tasks/{id}" => super::v1::get_task,
        put "/v1/tasks/{id}" => super::v1::update_task,
        delete "/v1/tasks/{id}" => super::v1::delete_task,
        // Notes
        get "/v1/notes" => super::v1::list_notes,
        get "/v1/notes/{id}" => super::v1::get_note,
        post "/v1/notes" => super::v1::create_note,
        put "/v1/notes/{id}" => super::v1::update_note,
        delete "/v1/notes/{id}" => super::v1::delete_note,
    });

    system_routes
        .merge(v1_routes)
        .merge(Scalar::with_url("/docs", api))
        .with_state(state)
}
