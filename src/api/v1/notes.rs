//! Note management handlers.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

use crate::api::AppState;
use crate::db::{Database, DbError, ListQuery, Note, NoteRepository, NoteType, SortOrder};

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
    pub created_at: String,
    pub updated_at: String,
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
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListNotesQuery {
    /// FTS5 search query (optional)
    #[param(example = "rust programming")]
    pub q: Option<String>,
    /// Filter by tags (comma-separated)
    #[param(example = "api,session")]
    pub tags: Option<String>,
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
    path = "/v1/notes",
    tag = "notes",
    params(ListNotesQuery),
    responses(
        (status = 200, description = "Paginated list of notes", body = PaginatedNotes),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn list_notes<D: Database>(
    State(state): State<AppState<D>>,
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

    let db_query = ListQuery {
        limit: query.limit,
        offset: query.offset,
        sort_by: query.sort.clone(),
        sort_order: match query.order.as_deref() {
            Some("desc") => Some(SortOrder::Desc),
            Some("asc") => Some(SortOrder::Asc),
            _ => None,
        },
        tags,
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
                .search_paginated(q, &db_query)
                .map_err(internal_error)?
        }
    } else {
        state
            .db()
            .notes()
            .list_paginated(&db_query)
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
    path = "/v1/notes/{id}",
    tag = "notes",
    params(("id" = String, Path, description = "Note ID")),
    responses(
        (status = 200, description = "Note found", body = NoteResponse),
        (status = 404, description = "Note not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn get_note<D: Database>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Json<NoteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let note = state.db().notes().get(&id).map_err(|e| match e {
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
    path = "/v1/notes",
    tag = "notes",
    request_body = CreateNoteRequest,
    responses(
        (status = 201, description = "Note created", body = NoteResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn create_note<D: Database>(
    State(state): State<AppState<D>>,
    Json(req): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<NoteResponse>), (StatusCode, Json<ErrorResponse>)> {
    let id = format!("{:08x}", rand_id());
    let now = chrono_now();

    let note_type = req
        .note_type
        .as_deref()
        .map(parse_note_type)
        .unwrap_or(NoteType::Manual);

    let note = Note {
        id: id.clone(),
        title: req.title,
        content: req.content,
        tags: req.tags,
        note_type,
        created_at: now.clone(),
        updated_at: now,
    };

    state.db().notes().create(&note).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(NoteResponse::from(note))))
}

#[utoipa::path(
    put,
    path = "/v1/notes/{id}",
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
pub async fn update_note<D: Database>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNoteRequest>,
) -> Result<Json<NoteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut note = state.db().notes().get(&id).map_err(|e| match e {
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

    if let Some(note_type_str) = req.note_type {
        note.note_type = parse_note_type(&note_type_str);
    }

    state.db().notes().update(&note).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(NoteResponse::from(note)))
}

#[utoipa::path(
    delete,
    path = "/v1/notes/{id}",
    tag = "notes",
    params(("id" = String, Path, description = "Note ID")),
    responses(
        (status = 204, description = "Note deleted"),
        (status = 404, description = "Note not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn delete_note<D: Database>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state.db().notes().delete(&id).map_err(|e| match e {
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

fn rand_id() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (duration.as_secs() as u32) ^ (duration.subsec_nanos())
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = secs / 86400;
    let years = 1970 + (days / 365);
    format!("{}-01-01 00:00:00", years)
}
