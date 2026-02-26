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
use crate::api::notifier::UpdateMessage;
use crate::db::{Database, DbError, PageSort, Skill, SkillQuery, SkillRepository, SortOrder};

use super::ErrorResponse;

// =============================================================================
// DTOs
// =============================================================================

#[derive(Serialize, ToSchema)]
pub struct SkillResponse {
    #[schema(example = "skl00123")]
    pub id: String,
    #[schema(example = "deploy-kubernetes")]
    pub name: String,
    #[schema(example = "Deploy apps to K8s cluster")]
    pub description: String,
    #[schema(
        example = "---\nname: deploy-kubernetes\ndescription: Deploy apps\n---\n# Instructions"
    )]
    pub content: String,
    pub tags: Vec<String>,
    pub project_ids: Vec<String>,
    pub scripts: Vec<String>,
    pub references: Vec<String>,
    pub assets: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Skill> for SkillResponse {
    fn from(s: Skill) -> Self {
        Self {
            id: s.id,
            name: s.name,
            description: s.description,
            content: s.content,
            tags: s.tags,
            project_ids: s.project_ids,
            scripts: s.scripts,
            references: s.references,
            assets: s.assets,
            created_at: s.created_at.unwrap_or_default(),
            updated_at: s.updated_at.unwrap_or_default(),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedSkills {
    pub items: Vec<SkillResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListSkillsQuery {
    /// FTS5 search query (optional). If provided, performs full-text search.
    #[param(example = "rust AND async")]
    #[serde(rename = "q")]
    pub query: Option<String>,
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
    #[schema(example = "deploy-kubernetes")]
    pub name: String,
    #[schema(example = "Deploy apps to K8s cluster")]
    pub description: String,
    #[schema(
        example = "---\nname: deploy-kubernetes\ndescription: Deploy apps\n---\n# Instructions"
    )]
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub project_ids: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceSkillRequest {
    #[schema(example = "deploy-kubernetes")]
    pub name: String,
    #[schema(example = "Deploy apps to K8s cluster")]
    pub description: String,
    #[schema(
        example = "---\nname: deploy-kubernetes\ndescription: Deploy apps\n---\n# Instructions"
    )]
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub project_ids: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSkillRequest {
    #[schema(example = "deploy-kubernetes")]
    pub name: Option<String>,
    #[schema(example = "Deploy apps to K8s cluster")]
    pub description: Option<String>,
    #[schema(
        example = "---\nname: deploy-kubernetes\ndescription: Deploy apps\n---\n# Instructions"
    )]
    pub content: Option<String>,
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
    skill.content = req.content;
    skill.tags = req.tags;
    skill.project_ids = req.project_ids;
    skill.updated_at = None;
    repo.update(&skill).await.map_err(|e| match e {
        DbError::Validation { .. } => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    // Broadcast notification
    state.notifier().notify(UpdateMessage::SkillUpdated {
        skill_id: skill.id.clone(),
    });

    Ok(Json(SkillResponse::from(skill)))
}

#[utoipa::path(
    get,
    path = "/api/v1/skills",
    tag = "skills",
    params(ListSkillsQuery),
    responses(
        (status = 200, description = "List of skills", body = PaginatedSkills),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
/// List skills (with optional filtering/query)
pub async fn list_skills<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Query(api_query): Query<ListSkillsQuery>,
) -> Result<Json<PaginatedSkills>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db();
    let repo = db.skills();

    // Convert API query to DB query
    let tags = api_query.tags.as_ref().map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    });

    let db_query = SkillQuery {
        page: PageSort {
            limit: api_query.limit,
            offset: api_query.offset,
            sort_by: api_query.sort.clone(),
            sort_order: match api_query.order.as_deref() {
                Some("desc") => Some(SortOrder::Desc),
                Some("asc") => Some(SortOrder::Asc),
                _ => None,
            },
        },
        tags,
        project_id: api_query.project_id.clone(),
    };

    // Route to search if query is provided, otherwise list all
    let results = if let Some(q) = &api_query.query {
        if !q.trim().is_empty() {
            repo.search(q, Some(&db_query)).await
        } else {
            repo.list(Some(&db_query)).await
        }
    } else {
        repo.list(Some(&db_query)).await
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(PaginatedSkills {
        items: results.items.into_iter().map(SkillResponse::from).collect(),
        total: results.total,
        limit: results.limit.unwrap_or(50),
        offset: results.offset,
    }))
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
    Ok(Json(SkillResponse::from(skill)))
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
        content: req.content,
        tags: req.tags,
        project_ids: req.project_ids,
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: None,
        updated_at: None,
    };
    let created = repo.create(&skill).await.map_err(|e| match e {
        DbError::Validation { .. } => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    // Broadcast notification
    state.notifier().notify(UpdateMessage::SkillCreated {
        skill_id: created.id.clone(),
    });

    Ok((StatusCode::CREATED, Json(SkillResponse::from(created))))
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
        skill.description = description;
    }
    if let Some(content) = req.content {
        skill.content = content;
    }
    if let Some(tags) = req.tags {
        skill.tags = tags;
    }
    if let Some(project_ids) = req.project_ids {
        skill.project_ids = project_ids;
    }
    skill.updated_at = None;
    repo.update(&skill).await.map_err(|e| match e {
        DbError::Validation { .. } => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    // Broadcast notification
    state.notifier().notify(UpdateMessage::SkillUpdated {
        skill_id: skill.id.clone(),
    });

    Ok(Json(SkillResponse::from(skill)))
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

    // Broadcast notification
    state.notifier().notify(UpdateMessage::SkillDeleted {
        skill_id: skill_id.clone(),
    });

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// Import Skill
// =============================================================================

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportSkillRequest {
    /// Source location (local path, git URL)
    #[schema(example = "./path/to/skill")]
    pub source: String,
    /// Optional subpath within source
    #[schema(example = "skills/deploy")]
    pub path: Option<String>,
    /// Project IDs to link
    pub project_ids: Option<Vec<String>>,
    /// Tags to apply to the skill
    pub tags: Option<Vec<String>>,
    /// If true, update existing skill; if false, fail on duplicate
    #[serde(default)]
    pub update: bool,
}

/// Import a skill from a source (local filesystem or git repository)
#[utoipa::path(
    post,
    path = "/api/v1/skills/import",
    tag = "skills",
    request_body = ImportSkillRequest,
    responses(
        (status = 201, description = "Skill imported successfully", body = SkillResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Import failed", body = ErrorResponse)
    )
)]
pub async fn import_skill<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Json(req): Json<ImportSkillRequest>,
) -> Result<(StatusCode, Json<SkillResponse>), (StatusCode, Json<ErrorResponse>)> {
    let db = state.db();

    // Call the import function from skills module
    let skill = crate::skills::import_skill(
        db,
        &req.source,
        req.path.as_deref(),
        req.project_ids,
        req.tags,
        req.update,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Import failed: {}", e),
            }),
        )
    })?;

    // Auto-enable: Extract attachments to cache immediately after import
    let skill_name = crate::skills::parse_skill_name_from_content(&skill.content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to parse skill name: {}", e),
            }),
        )
    })?;

    let attachments = db.skills().get_attachments(&skill.id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get attachments: {}", e),
            }),
        )
    })?;

    // Extract to cache (ignore errors - cache is best-effort)
    let _ = crate::skills::extract_attachments(
        state.skills_dir(),
        &skill_name,
        &skill.content,
        &attachments,
    );

    // Broadcast notification
    state.notifier().notify(UpdateMessage::SkillCreated {
        skill_id: skill.id.clone(),
    });

    Ok((StatusCode::CREATED, Json(skill.into())))
}

// =============================================================================
// Enable/Disable Cache Management
// =============================================================================

#[derive(Serialize, ToSchema)]
pub struct EnableSkillResponse {
    pub skill_id: String,
    pub skill_name: String,
    pub cache_path: String,
}

#[derive(Serialize, ToSchema)]
pub struct DisableSkillResponse {
    pub skill_id: String,
    pub skill_name: String,
}

/// Enable a skill by extracting attachments to cache
///
/// Supports lookup by ID or name. Extracts SKILL.md and all attachments
/// to the cache directory.
#[utoipa::path(
    post,
    path = "/api/v1/skills/{id_or_name}/enable",
    tag = "skills",
    params(("id_or_name" = String, Path, description = "Skill ID or name")),
    responses(
        (status = 200, description = "Skill enabled", body = EnableSkillResponse),
        (status = 404, description = "Skill not found", body = ErrorResponse),
        (status = 500, description = "Enable failed", body = ErrorResponse)
    )
)]
pub async fn enable_skill<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id_or_name): Path<String>,
) -> Result<Json<EnableSkillResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db();
    let repo = db.skills();

    // Try to get skill by ID first, then by name
    let skill = match repo.get(&id_or_name).await {
        Ok(s) => s,
        Err(DbError::NotFound { .. }) => {
            // Try by name
            let query = crate::db::SkillQuery {
                page: crate::db::PageSort {
                    limit: Some(1),
                    offset: None,
                    sort_by: None,
                    sort_order: None,
                },
                tags: None,
                project_id: None,
            };
            let results = repo.list(Some(&query)).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
            })?;

            // Filter by name (case-sensitive)
            results
                .items
                .into_iter()
                .find(|s| s.name == id_or_name)
                .ok_or_else(|| {
                    (
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse {
                            error: format!("Skill '{}' not found", id_or_name),
                        }),
                    )
                })?
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ));
        }
    };

    // Parse skill name from content
    let skill_name = crate::skills::parse_skill_name_from_content(&skill.content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to parse skill name: {}", e),
            }),
        )
    })?;

    // Get attachments
    let attachments = repo.get_attachments(&skill.id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get attachments: {}", e),
            }),
        )
    })?;

    // Extract to cache
    let cache_path = crate::skills::extract_attachments(
        state.skills_dir(),
        &skill_name,
        &skill.content,
        &attachments,
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to extract attachments: {}", e),
            }),
        )
    })?;

    Ok(Json(EnableSkillResponse {
        skill_id: skill.id,
        skill_name,
        cache_path: cache_path.to_string_lossy().to_string(),
    }))
}

/// Disable a skill by invalidating its cache
///
/// Supports lookup by ID or name. Removes all cached files for the skill.
#[utoipa::path(
    post,
    path = "/api/v1/skills/{id_or_name}/disable",
    tag = "skills",
    params(("id_or_name" = String, Path, description = "Skill ID or name")),
    responses(
        (status = 200, description = "Skill disabled", body = DisableSkillResponse),
        (status = 404, description = "Skill not found", body = ErrorResponse),
        (status = 500, description = "Disable failed", body = ErrorResponse)
    )
)]
pub async fn disable_skill<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id_or_name): Path<String>,
) -> Result<Json<DisableSkillResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db();
    let repo = db.skills();

    // Try to get skill by ID first, then by name
    let skill = match repo.get(&id_or_name).await {
        Ok(s) => s,
        Err(DbError::NotFound { .. }) => {
            // Try by name
            let query = crate::db::SkillQuery {
                page: crate::db::PageSort {
                    limit: Some(1),
                    offset: None,
                    sort_by: None,
                    sort_order: None,
                },
                tags: None,
                project_id: None,
            };
            let results = repo.list(Some(&query)).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
            })?;

            // Filter by name (case-sensitive)
            results
                .items
                .into_iter()
                .find(|s| s.name == id_or_name)
                .ok_or_else(|| {
                    (
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse {
                            error: format!("Skill '{}' not found", id_or_name),
                        }),
                    )
                })?
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ));
        }
    };

    // Parse skill name from content
    let skill_name = crate::skills::parse_skill_name_from_content(&skill.content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to parse skill name: {}", e),
            }),
        )
    })?;

    // Invalidate cache
    crate::skills::invalidate_cache(&skill_name).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to invalidate cache: {}", e),
            }),
        )
    })?;

    Ok(Json(DisableSkillResponse {
        skill_id: skill.id,
        skill_name,
    }))
}
