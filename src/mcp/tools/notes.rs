//! MCP tools for Note management.

use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    schemars::JsonSchema,
    tool, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::api::notifier::{ChangeNotifier, UpdateMessage};
use crate::db::{Database, Note, NoteQuery, NoteRepository, NoteType, PageSort};
use crate::mcp::tools::{apply_limit, map_db_error};

// =============================================================================
// Parameter Structs
// =============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListNotesParams {
    #[schemars(
        description = "Filter by note type: 'manual' (user notes) or 'archived_todo' (completed tasks)"
    )]
    pub note_type: Option<String>,
    #[schemars(
        description = "Filter by tags. Use reference tags to find linked notes: ['parent:NOTE_ID'], ['related:NOTE_ID']"
    )]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Filter by project ID")]
    pub project_id: Option<String>,
    #[schemars(description = "Maximum number of items to return (default: 10, max: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of items to skip")]
    pub offset: Option<usize>,
    #[schemars(
        description = "Include note content in response (default: false for lighter list responses). Set to true to retrieve full content."
    )]
    pub include_content: Option<bool>,
    #[schemars(
        description = "Field to sort by (title, created_at, updated_at). Default: created_at"
    )]
    pub sort: Option<String>,
    #[schemars(description = "Sort order (asc, desc). Default: asc")]
    pub order: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetNoteParams {
    #[schemars(description = "Note ID")]
    pub note_id: String,
    #[schemars(
        description = "Include note content in response (default: true). Set to false to retrieve only metadata."
    )]
    pub include_content: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateNoteParams {
    #[schemars(description = "Note title")]
    pub title: String,
    #[schemars(
        description = "Note content (Markdown supported). KEEP UNDER 10k chars to avoid context overflow. Larger content? Create new note with 'parent:THIS_ID' tag."
    )]
    pub content: String,
    #[schemars(
        description = "Tags for organization. Use 'parent:NOTE_ID' for continuations, 'related:NOTE_ID' for references, 'session' for persistent session notes. CRITICAL: Session notes MUST be re-read after context compaction to restore state."
    )]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "Note type: 'manual' (default, user notes) or 'archived_todo' (system-generated from completed tasks)"
    )]
    pub note_type: Option<String>,
    #[schemars(
        description = "Repository IDs to link (optional). Associate with relevant repos for context."
    )]
    pub repo_ids: Option<Vec<String>>,
    #[schemars(
        description = "Project IDs to link (RECOMMENDED). Attach to relevant project for organization and discoverability. REQUIRED for session notes - always link session notes to their project(s)."
    )]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateNoteParams {
    #[schemars(description = "Note ID")]
    pub note_id: String,
    #[schemars(description = "Note title (optional)")]
    pub title: Option<String>,
    #[schemars(
        description = "Note content (optional). KEEP UNDER 10k chars. If note is getting large, create continuation note with 'parent:THIS_ID' tag instead."
    )]
    pub content: Option<String>,
    #[schemars(
        description = "Tags (optional). Use 'parent:NOTE_ID' for continuations, 'related:NOTE_ID' for references. Replaces all existing tags when provided."
    )]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "Repository IDs to link (optional). Associate with relevant repos for context."
    )]
    pub repo_ids: Option<Vec<String>>,
    #[schemars(
        description = "Project IDs to link (RECOMMENDED). Attach to relevant project for organization and discoverability. REQUIRED for session notes - always link session notes to their project(s)."
    )]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteNoteParams {
    #[schemars(description = "Note ID")]
    pub note_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SearchNotesParams {
    #[schemars(
        description = "FTS5 search query. Examples: 'rust AND async' (Boolean), '\"exact phrase\"' (phrase match), 'term*' (prefix), 'NOT deprecated' (exclude), 'api AND (error OR bug)' (complex)"
    )]
    pub query: String,
    #[schemars(
        description = "Filter results by tags (optional). Can combine with search to find e.g. session notes matching a term."
    )]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Filter by project ID (optional)")]
    pub project_id: Option<String>,
    #[schemars(description = "Maximum number of results to return (default: 10, max: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of results to skip (optional)")]
    pub offset: Option<usize>,
    #[schemars(
        description = "Field to sort by (title, created_at, updated_at). Default: created_at"
    )]
    pub sort: Option<String>,
    #[schemars(description = "Sort order (asc, desc). Default: asc")]
    pub order: Option<String>,
}

// =============================================================================
// Note Tools
// =============================================================================

#[derive(Clone)]
pub struct NoteTools<D: Database> {
    db: Arc<D>,
    notifier: ChangeNotifier,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> NoteTools<D> {
    pub fn new(db: Arc<D>, notifier: ChangeNotifier) -> Self {
        Self {
            db,
            notifier,
            tool_router: Self::tool_router(),
        }
    }

    /// Get the tool router for this handler
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    #[tool(
        description = "List notes with optional filtering and sorting. Default excludes content (metadata only) - use include_content=true for full notes. Filter by tags, project_id, or note_type. Sort by title, created_at, or updated_at. Limit default: 10, max: 20."
    )]
    pub async fn list_notes(
        &self,
        params: Parameters<ListNotesParams>,
    ) -> Result<CallToolResult, McpError> {
        // Default to include_content=false for lighter list responses
        let include_content = params.0.include_content.unwrap_or(false);

        // Build query
        let query = NoteQuery {
            page: PageSort {
                limit: params.0.limit,
                offset: params.0.offset,
                sort_by: params.0.sort.clone(),
                sort_order: match params.0.order.as_deref() {
                    Some("desc") => Some(crate::db::SortOrder::Desc),
                    Some("asc") => Some(crate::db::SortOrder::Asc),
                    _ => None,
                },
            },
            tags: params.0.tags.clone(),
            project_id: params.0.project_id.clone(),
        };

        let result = if include_content {
            self.db.notes().list(Some(&query)).await
        } else {
            self.db.notes().list_metadata_only(Some(&query)).await
        }
        .map_err(map_db_error)?;

        // Filter by note_type if specified (note_type not in NoteQuery yet)
        let filtered_items: Vec<Note> = if let Some(note_type_str) = &params.0.note_type {
            let note_type = note_type_str.parse::<NoteType>().map_err(|e| {
                McpError::invalid_params("invalid_note_type", Some(serde_json::json!({"error": e})))
            })?;
            result
                .items
                .into_iter()
                .filter(|note| note.note_type == note_type)
                .collect()
        } else {
            result.items
        };

        let response = json!({
            "items": filtered_items,
            "total": filtered_items.len(),
            "limit": result.limit,
            "offset": result.offset,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap(),
        )]))
    }

    #[tool(
        description = "Get a note by ID. Returns full content by default - set include_content=false for metadata only."
    )]
    pub async fn get_note(
        &self,
        params: Parameters<GetNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        // Default to include_content=true for backward compatibility
        let include_content = params.0.include_content.unwrap_or(true);

        let note = if include_content {
            self.db.notes().get(&params.0.note_id).await
        } else {
            self.db.notes().get_metadata_only(&params.0.note_id).await
        }
        .map_err(|e| {
            McpError::resource_not_found(
                "note_not_found",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&note).unwrap(),
        )]))
    }

    #[tool(
        description = "Create a new note (Markdown supported). IMPORTANT: Keep under 10k chars (~2.5k tokens) to avoid context overflow. For larger content, split into multiple notes and link with tags: 'parent:NOTE_ID' (continuation), 'related:NOTE_ID' (reference). Link to projects/repos via project_ids/repo_ids."
    )]
    pub async fn create_note(
        &self,
        params: Parameters<CreateNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        // Parse note_type
        let note_type = if let Some(nt_str) = &params.0.note_type {
            nt_str.parse::<NoteType>().map_err(|e| {
                McpError::invalid_params("invalid_note_type", Some(serde_json::json!({"error": e})))
            })?
        } else {
            NoteType::Manual
        };

        let note = Note {
            id: String::new(), // Will be generated by DB
            title: params.0.title.clone(),
            content: params.0.content.clone(),
            tags: params.0.tags.clone().unwrap_or_default(),
            note_type,
            repo_ids: params.0.repo_ids.clone().unwrap_or_default(),
            project_ids: params.0.project_ids.clone().unwrap_or_default(),
            created_at: None, // Will be set by DB
            updated_at: None, // Will be set by DB
        };

        let created = self.db.notes().create(&note).await.map_err(map_db_error)?;

        // Broadcast NoteCreated notification
        self.notifier.notify(UpdateMessage::NoteCreated {
            note_id: created.id.clone(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&created).unwrap(),
        )]))
    }

    #[tool(
        description = "Update an existing note. All fields optional - only provided fields are updated. IMPORTANT: Keep under 10k chars. To add content without exceeding limit, create a new note with 'parent:THIS_ID' tag instead of updating."
    )]
    pub async fn update_note(
        &self,
        params: Parameters<UpdateNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        // Get existing note
        let mut note = self
            .db
            .notes()
            .get(&params.0.note_id)
            .await
            .map_err(map_db_error)?;

        // Update fields
        if let Some(title) = &params.0.title {
            note.title = title.clone();
        }
        if let Some(content) = &params.0.content {
            note.content = content.clone();
        }
        if let Some(tags) = &params.0.tags {
            note.tags = tags.clone();
        }
        if let Some(repo_ids) = &params.0.repo_ids {
            note.repo_ids = repo_ids.clone();
        }
        if let Some(project_ids) = &params.0.project_ids {
            note.project_ids = project_ids.clone();
        }

        self.db.notes().update(&note).await.map_err(map_db_error)?;

        // Fetch updated note to get auto-set updated_at
        let updated = self.db.notes().get(&params.0.note_id).await.map_err(|e| {
            McpError::internal_error(
                "database_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Broadcast NoteUpdated notification
        self.notifier.notify(UpdateMessage::NoteUpdated {
            note_id: params.0.note_id.clone(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&updated).unwrap(),
        )]))
    }

    #[tool(
        description = "Delete a note permanently. Use sparingly - consider archiving via tags instead."
    )]
    pub async fn delete_note(
        &self,
        params: Parameters<DeleteNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .notes()
            .delete(&params.0.note_id)
            .await
            .map_err(map_db_error)?;

        // Broadcast NoteDeleted notification
        self.notifier.notify(UpdateMessage::NoteDeleted {
            note_id: params.0.note_id.clone(),
        });

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Note {} deleted successfully",
            params.0.note_id
        ))]))
    }

    #[tool(
        description = "Full-text search notes (FTS5) with optional sorting. Supports: 'rust AND async' (Boolean), '\"exact phrase\"' (phrase), 'term*' (prefix), 'NOT deprecated' (exclusion). Filter results by tags/project_id. Sort by title, created_at, or updated_at. Returns metadata only (no content)."
    )]
    pub async fn search_notes(
        &self,
        params: Parameters<SearchNotesParams>,
    ) -> Result<CallToolResult, McpError> {
        // Build query
        let query = NoteQuery {
            page: PageSort {
                limit: Some(apply_limit(params.0.limit)),
                offset: params.0.offset,
                sort_by: params.0.sort.clone(),
                sort_order: match params.0.order.as_deref() {
                    Some("desc") => Some(crate::db::SortOrder::Desc),
                    Some("asc") => Some(crate::db::SortOrder::Asc),
                    _ => None,
                },
            },
            tags: params.0.tags.clone(),
            project_id: params.0.project_id.clone(),
        };

        let result = self
            .db
            .notes()
            .search(&params.0.query, Some(&query))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "database_error",
                    Some(serde_json::json!({"error": e.to_string()})),
                )
            })?;

        let response = json!({
            "items": result.items,
            "total": result.total,
            "limit": result.limit,
            "offset": result.offset,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap(),
        )]))
    }
}
