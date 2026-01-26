//! Skill management handlers

use crate::sync::GitOps;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::api::AppState;
use crate::db::{Database, DbError, PageSort, Skill, SkillQuery, SkillRepository, SortOrder};

use super::ErrorResponse;

// =============================================================================
// DTOs
// =============================================================================

#[derive(Serialize, ToSchema)]
pub struct SkillResponse {
    #[schema(example = "skl00123")]
    pub id: String,
    #[schema(example = "Rust")]
    pub name: String,
    #[schema(example = "Low-level systems programming")]
    pub description: Option<String>,
    #[schema(example = "Follow the Rust Book")]
    pub instructions: Option<String>,
    pub tags: Vec<String>,
    pub project_ids: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl From<Skill> for SkillResponse {
    fn from(s: Skill) -> Self {
        Self {
            id: s.id,
            name: s.name,
            description: s.description,
            instructions: s.instructions,
            tags: s.tags,
            project_ids: s.project_ids,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListSkillsQuery {
    /// Filter by tags (comma-separated)
    #[param(example = "rust,programming")]
    pub tags: Option<String>,
    /// Filter by project ID
    #[param(example = "a1b2c3d4")]
    pub project_id: Option<String>,
    /// Maximum number of items to return
    #[param(example = 20)]
    pub limit: Option<usize>,
    /// Number of items to skip
    #[param(example = 0)]
    pub offset: Option<usize>,
    /// Field to sort by (name, created_at)
    #[param(example = "created_at")]
    pub sort: Option<String>,
    /// Sort order (asc, desc)
    #[param(example = "desc")]
    pub order: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSkillRequest {
    #[schema(example = "Rust")]
    pub name: String,
    #[schema(example = "Low-level systems programming")]
    pub description: Option<String>,
    #[schema(example = "Follow the Rust Book")]
    pub instructions: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub project_ids: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceSkillRequest {
    #[schema(example = "Rust")]
    pub name: String,
    #[schema(example = "Low-level systems programming")]
    pub description: Option<String>,
    #[schema(example = "Follow the Rust Book")]
    pub instructions: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub project_ids: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSkillRequest {
    #[schema(example = "Rust")]
    pub name: Option<String>,
    #[schema(example = "Low-level systems programming")]
    pub description: Option<String>,
    #[schema(example = "Follow the Rust Book")]
    pub instructions: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub project_ids: Option<Vec<String>>,
}

// =============================================================================
// Handlers
// =============================================================================

#[utoipa::path(
    put,
    path = "/api/v1/skills/{id}",
    tag = "skills",
    params(("id" = String, Path, description = "Skill ID")),
    request_body = ReplaceSkillRequest,
    responses(
        (status = 200, description = "Skill updated", body = SkillResponse),
        (status = 404, description = "Skill not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
/// Replace (fully update) an existing skill by ID (PUT)
pub async fn replace_skill<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(skill_id): Path<String>,
    Json(req): Json<ReplaceSkillRequest>,
) -> Result<Json<SkillResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db();
    let repo = db.skills();
    let mut skill = repo.get(&skill_id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Skill '{}' not found", skill_id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;
    skill.name = req.name;
    skill.description = req.description;
    skill.instructions = req.instructions;
    skill.tags = req.tags;
    skill.project_ids = req.project_ids;
    repo.update(&skill).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(Json(SkillResponse {
        id: skill.id,
        name: skill.name,
        description: skill.description,
        instructions: skill.instructions,
        tags: skill.tags,
        project_ids: skill.project_ids,
        created_at: skill.created_at,
        updated_at: skill.updated_at,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/skills",
    tag = "skills",
    params(ListSkillsQuery),
    responses(
        (status = 200, description = "List of skills", body = [SkillResponse]),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
/// List skills (with optional filtering/query)
pub async fn list_skills<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Query(query): Query<ListSkillsQuery>,
) -> Result<Json<Vec<SkillResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db();
    let repo = db.skills();

    // Convert API query to DB query
    let tags = query.tags.as_ref().map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    });

    let db_query = SkillQuery {
        page: PageSort {
            limit: query.limit,
            offset: query.offset,
            sort_by: query.sort.clone(),
            sort_order: match query.order.as_deref() {
                Some("desc") => Some(SortOrder::Desc),
                Some("asc") => Some(SortOrder::Asc),
                _ => None,
            },
        },
        tags,
        project_id: query.project_id.clone(),
    };

    let results = repo.list(Some(&db_query)).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(
        results.items.into_iter().map(SkillResponse::from).collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/skills/{id}",
    tag = "skills",
    params(("id" = String, Path, description = "Skill ID")),
    responses(
        (status = 200, description = "Skill found", body = SkillResponse),
        (status = 404, description = "Skill not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
/// Get a skill by ID
pub async fn get_skill<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(skill_id): Path<String>,
) -> Result<Json<SkillResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db();
    let repo = db.skills();
    let skill = repo.get(&skill_id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Skill '{}' not found", skill_id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;
    Ok(Json(SkillResponse {
        id: skill.id,
        name: skill.name,
        description: skill.description,
        instructions: skill.instructions,
        tags: skill.tags,
        project_ids: skill.project_ids,
        created_at: skill.created_at,
        updated_at: skill.updated_at,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/skills",
    tag = "skills",
    request_body = CreateSkillRequest,
    responses(
        (status = 201, description = "Skill created", body = SkillResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
/// Create a new skill
pub async fn create_skill<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Json(req): Json<CreateSkillRequest>,
) -> Result<(StatusCode, Json<SkillResponse>), (StatusCode, Json<ErrorResponse>)> {
    let db = state.db();
    let repo = db.skills();
    let skill = Skill {
        id: crate::db::utils::generate_entity_id(),
        name: req.name,
        description: req.description,
        instructions: req.instructions,
        tags: req.tags,
        project_ids: req.project_ids,
        created_at: None,
        updated_at: None,
    };
    let created = repo.create(&skill).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;
    Ok((
        StatusCode::CREATED,
        Json(SkillResponse {
            id: created.id,
            name: created.name,
            description: created.description,
            instructions: created.instructions,
            tags: created.tags,
            project_ids: created.project_ids,
            created_at: created.created_at,
            updated_at: created.updated_at,
        }),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/v1/skills/{id}",
    tag = "skills",
    params(("id" = String, Path, description = "Skill ID")),
    request_body = UpdateSkillRequest,
    responses(
        (status = 200, description = "Skill partially updated", body = SkillResponse),
        (status = 404, description = "Skill not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
/// Update an existing skill
pub async fn patch_skill<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(skill_id): Path<String>,
    Json(req): Json<UpdateSkillRequest>,
) -> Result<Json<SkillResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db();
    let repo = db.skills();
    let mut skill = repo.get(&skill_id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Skill '{}' not found", skill_id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;
    if let Some(name) = req.name {
        skill.name = name;
    }
    if let Some(description) = req.description {
        skill.description = Some(description);
    }
    if let Some(instructions) = req.instructions {
        skill.instructions = Some(instructions);
    }
    if let Some(tags) = req.tags {
        skill.tags = tags;
    }
    if let Some(project_ids) = req.project_ids {
        skill.project_ids = project_ids;
    }
    repo.update(&skill).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(Json(SkillResponse {
        id: skill.id,
        name: skill.name,
        description: skill.description,
        instructions: skill.instructions,
        tags: skill.tags,
        project_ids: skill.project_ids,
        created_at: skill.created_at,
        updated_at: skill.updated_at,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/skills/{id}",
    tag = "skills",
    params(("id" = String, Path, description = "Skill ID")),
    responses(
        (status = 204, description = "Skill deleted"),
        (status = 404, description = "Skill not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
/// Delete a skill
pub async fn delete_skill<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(skill_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db();
    let repo = db.skills();
    repo.delete(&skill_id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Skill '{}' not found", skill_id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;
    Ok(StatusCode::NO_CONTENT)
}
