//! API route configuration.

use axum::Router;
use axum::routing::{delete, get, post, put};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use super::handlers::{
    self, CreateProjectRequest, ErrorResponse, HealthResponse, ProjectResponse,
    UpdateProjectRequest,
};
use super::state::AppState;
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
        handlers::list_projects,
        handlers::get_project,
        handlers::create_project,
        handlers::update_project,
        handlers::delete_project,
    ),
    components(
        schemas(
            HealthResponse,
            ProjectResponse,
            CreateProjectRequest,
            UpdateProjectRequest,
            ErrorResponse,
        )
    ),
    tags(
        (name = "system", description = "System health and status endpoints"),
        (name = "projects", description = "Project management endpoints")
    )
)]
pub struct ApiDoc;

/// Create the API router with OpenAPI documentation
pub fn create_router<D: Database + 'static>(state: AppState<D>) -> Router {
    let api = ApiDoc::openapi();

    // System routes (non-generic)
    let system_routes = Router::new()
        .route("/", get(handlers::root))
        .route("/health", get(handlers::health));

    // Project routes (generic over Database)
    let project_routes = routes!(D => {
        get "/projects" => handlers::list_projects,
        get "/projects/{id}" => handlers::get_project,
        post "/projects" => handlers::create_project,
        put "/projects/{id}" => handlers::update_project,
        delete "/projects/{id}" => handlers::delete_project,
    });

    system_routes
        .merge(project_routes)
        .merge(Scalar::with_url("/docs", api))
        .with_state(state)
}
