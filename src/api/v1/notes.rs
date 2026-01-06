//! Note management handlers.

use crate::sync::GitOps;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

use crate::api::AppState;
use crate::api::notifier::UpdateMessage;
use crate::db::{
    Database, DbError, Note, NoteQuery, NoteRepository, NoteType, PageSort, SortOrder,
};

use super::ErrorResponse;

// =============================================================================
// DTOs
// =============================================================================

#[derive(Serialize, ToSchema)]
pub struct NoteResponse {
    #[schema(example = "a1b2c3d4")]
    pub id: String,
    #[schema(example = "My Note")]
    pub title: String,
    #[schema(example = "Note content in markdown")]
    pub content: String,
    pub tags: Vec<String>,
    #[schema(example = "manual")]
    pub note_type: String,
    /// Linked repository IDs (M:N relationship via note_repo)
    #[schema(example = json!(["repo123a", "repo456b"]))]
    pub repo_ids: Vec<String>,
    /// Linked project IDs (M:N relationship via project_note)
    #[schema(example = json!(["proj123a", "proj456b"]))]
    pub project_ids: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl From<Note> for NoteResponse {
    fn from(n: Note) -> Self {
        Self {
            id: n.id,
            title: n.title,
            content: n.content,
            tags: n.tags,
            note_type: match n.note_type {
                NoteType::Manual => "manual",
                NoteType::ArchivedTodo => "archived_todo",
                NoteType::Scratchpad => "scratchpad",
            }
            .to_string(),
            repo_ids: n.repo_ids,
            project_ids: n.project_ids,
            created_at: n.created_at,
            updated_at: n.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateNoteRequest {
    #[schema(example = "My Note")]
    pub title: String,
    #[schema(example = "Note content in markdown")]
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[schema(example = "manual")]
    pub note_type: Option<String>,
    /// Linked repository IDs (M:N relationship via note_repo)
    #[schema(example = json!(["repo123a", "repo456b"]))]
    #[serde(default)]
    pub repo_ids: Vec<String>,
    /// Linked project IDs (M:N relationship via project_note)
    #[schema(example = json!(["proj123a", "proj456b"]))]
    #[serde(default)]
    pub project_ids: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateNoteRequest {
    #[schema(example = "Updated Note")]
    pub title: String,
    #[schema(example = "Updated content")]
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[schema(example = "manual")]
    pub note_type: Option<String>,
    /// Linked repository IDs (M:N relationship via note_repo)
    #[schema(example = json!(["repo123a", "repo456b"]))]
    #[serde(default)]
    pub repo_ids: Vec<String>,
    /// Linked project IDs (M:N relationship via project_note)
    #[schema(example = json!(["proj123a", "proj456b"]))]
    #[serde(default)]
    pub project_ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct PatchNoteRequest {
    #[schema(example = "Updated Note")]
    pub title: Option<String>,
    #[schema(example = "Updated content")]
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
    #[schema(example = "manual")]
    pub note_type: Option<String>,
    /// Linked repository IDs (M:N relationship via note_repo)
    #[schema(example = json!(["repo123a", "repo456b"]))]
    pub repo_ids: Option<Vec<String>>,
    /// Linked project IDs (M:N relationship via project_note)
    #[schema(example = json!(["proj123a", "proj456b"]))]
    pub project_ids: Option<Vec<String>>,
}

impl PatchNoteRequest {
    fn merge_into(self, target: &mut Note) {
        if let Some(title) = self.title {
            target.title = title;
        }
        if let Some(content) = self.content {
            target.content = content;
        }
        if let Some(tags) = self.tags {
            target.tags = tags;
        }
        if let Some(note_type_str) = self.note_type
            && let Ok(note_type) = note_type_str.parse::<NoteType>()
        {
            target.note_type = note_type;
        }
        if let Some(repo_ids) = self.repo_ids {
            target.repo_ids = repo_ids;
        }
        if let Some(project_ids) = self.project_ids {
            target.project_ids = project_ids;
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListNotesQuery {
    /// FTS5 search query (optional)
    #[param(example = "rust programming")]
    pub q: Option<String>,
    /// Filter by tags (comma-separated)
    #[param(example = "api,session")]
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
    /// Field to sort by (title, note_type, created_at, updated_at)
    #[param(example = "created_at")]
    pub sort: Option<String>,
    /// Sort order (asc, desc)
    #[param(example = "desc")]
    pub order: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedNotes {
    pub items: Vec<NoteResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

// =============================================================================
// Handlers
// =============================================================================

#[utoipa::path(
    get,
    path = "/api/v1/notes",
    tag = "notes",
    params(ListNotesQuery),
    responses(
        (status = 200, description = "Paginated list of notes", body = PaginatedNotes),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn list_notes<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Query(query): Query<ListNotesQuery>,
) -> Result<Json<PaginatedNotes>, (StatusCode, Json<ErrorResponse>)> {
    let internal_error = |e: crate::db::DbError| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    };

    // Build database query with tag filtering at DB level
    let tags = query.tags.as_ref().map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    });

    let db_query = NoteQuery {
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
        parent_id: None,
    };

    // Get notes - either search or list all (at database level)
    let result = if let Some(q) = &query.q {
        if q.is_empty() {
            // Empty search returns empty result
            crate::db::ListResult {
                items: vec![],
                total: 0,
                limit: query.limit,
                offset: query.offset.unwrap_or(0),
            }
        } else {
            state
                .db()
                .notes()
                .search(q, Some(&db_query))
                .await
                .map_err(internal_error)?
        }
    } else {
        state
            .db()
            .notes()
            .list(Some(&db_query))
            .await
            .map_err(internal_error)?
    };

    let items: Vec<NoteResponse> = result.items.into_iter().map(NoteResponse::from).collect();

    Ok(Json(PaginatedNotes {
        items,
        total: result.total,
        limit: result.limit.unwrap_or(50),
        offset: result.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/notes/{id}",
    tag = "notes",
    params(("id" = String, Path, description = "Note ID")),
    responses(
        (status = 200, description = "Note found", body = NoteResponse),
        (status = 404, description = "Note not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn get_note<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<Json<NoteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let note = state.db().notes().get(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Note '{}' not found", id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    Ok(Json(NoteResponse::from(note)))
}

#[utoipa::path(
    post,
    path = "/api/v1/notes",
    tag = "notes",
    request_body = CreateNoteRequest,
    responses(
        (status = 201, description = "Note created", body = NoteResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn create_note<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Json(req): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<NoteResponse>), (StatusCode, Json<ErrorResponse>)> {
    let note_type = req
        .note_type
        .as_deref()
        .map(parse_note_type)
        .unwrap_or(NoteType::Manual);

    // Create note with placeholder values - repository will generate ID and timestamps
    let note = Note {
        id: String::new(), // Repository will generate this
        title: req.title,
        content: req.content,
        tags: req.tags,
        note_type,
        parent_id: None,
        idx: None,
        repo_ids: req.repo_ids,
        project_ids: req.project_ids,
        created_at: None, // Repository will generate this
        updated_at: None, // Repository will generate this
    };

    let created_note = state.db().notes().create(&note).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Broadcast notification
    state.notifier().notify(UpdateMessage::NoteCreated {
        note_id: created_note.id.clone(),
    });

    Ok((StatusCode::CREATED, Json(NoteResponse::from(created_note))))
}

#[utoipa::path(
    put,
    path = "/api/v1/notes/{id}",
    tag = "notes",
    params(("id" = String, Path, description = "Note ID")),
    request_body = UpdateNoteRequest,
    responses(
        (status = 200, description = "Note updated", body = NoteResponse),
        (status = 404, description = "Note not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn update_note<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNoteRequest>,
) -> Result<Json<NoteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut note = state.db().notes().get(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Note '{}' not found", id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    note.title = req.title;
    note.content = req.content;
    note.tags = req.tags;
    note.repo_ids = req.repo_ids;
    note.project_ids = req.project_ids;

    if let Some(note_type_str) = req.note_type {
        note.note_type = parse_note_type(&note_type_str);
    }

    state.db().notes().update(&note).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Broadcast notification
    state.notifier().notify(UpdateMessage::NoteUpdated {
        note_id: note.id.clone(),
    });

    Ok(Json(NoteResponse::from(note)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/notes/{id}",
    tag = "notes",
    params(("id" = String, Path, description = "Note ID")),
    request_body = PatchNoteRequest,
    responses(
        (status = 200, description = "Note partially updated", body = NoteResponse),
        (status = 404, description = "Note not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn patch_note<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
    Json(req): Json<PatchNoteRequest>,
) -> Result<Json<NoteResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Fetch existing note
    let mut note = state.db().notes().get(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Note '{}' not found", id),
            }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    // Merge PATCH changes
    req.merge_into(&mut note);

    // Save
    state.db().notes().update(&note).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Broadcast notification
    state.notifier().notify(UpdateMessage::NoteUpdated {
        note_id: note.id.clone(),
    });

    Ok(Json(NoteResponse::from(note)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/notes/{id}",
    tag = "notes",
    params(("id" = String, Path, description = "Note ID")),
    responses(
        (status = 204, description = "Note deleted"),
        (status = 404, description = "Note not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn delete_note<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state.db().notes().delete(&id).await.map_err(|e| match e {
        DbError::NotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Note '{}' not found", id),
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
    state.notifier().notify(UpdateMessage::NoteDeleted {
        note_id: id.clone(),
    });

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// Helpers
// =============================================================================

fn parse_note_type(s: &str) -> NoteType {
    match s {
        "archived_todo" => NoteType::ArchivedTodo,
        "scratchpad" => NoteType::Scratchpad,
        _ => NoteType::Manual,
    }
}
