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

use crate::db::{Database, Note, NoteQuery, NoteRepository, NoteType, PageSort};
use crate::mcp::tools::{apply_limit, map_db_error};

// =============================================================================
// Parameter Structs
// =============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListNotesParams {
    #[schemars(description = "Filter by note type (manual, archived_todo)")]
    pub note_type: Option<String>,
    #[schemars(description = "Filter by tags (comma-separated)")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Maximum number of items to return (default: 10, max: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of items to skip")]
    pub offset: Option<usize>,
    #[schemars(
        description = "Include note content in response (default: false for lighter list responses). Set to true to retrieve full content."
    )]
    pub include_content: Option<bool>,
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
        description = "Note content (Markdown supported). Size limits: warns at 10k chars (~2.5k tokens), soft max 50k chars (~12.5k tokens), hard max 100k chars (~25k tokens). Split large notes and link with tags."
    )]
    pub content: String,
    #[schemars(description = "Tags for organization (optional)")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Note type (manual, archived_todo) (optional, defaults to manual)")]
    pub note_type: Option<String>,
    #[schemars(description = "Repository IDs to link (optional)")]
    pub repo_ids: Option<Vec<String>>,
    #[schemars(description = "Project IDs to link (optional)")]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateNoteParams {
    #[schemars(description = "Note ID")]
    pub note_id: String,
    #[schemars(description = "Note title (optional)")]
    pub title: Option<String>,
    #[schemars(
        description = "Note content (optional). Size limits: warns at 10k chars (~2.5k tokens), soft max 50k chars (~12.5k tokens), hard max 100k chars (~25k tokens). Split large notes and link with tags."
    )]
    pub content: Option<String>,
    #[schemars(description = "Tags for organization (optional)")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Repository IDs to link (optional)")]
    pub repo_ids: Option<Vec<String>>,
    #[schemars(description = "Project IDs to link (optional)")]
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
        description = "FTS5 search query (e.g., 'rust AND async', '\"exact phrase\"', 'term*')"
    )]
    pub query: String,
    #[schemars(description = "Filter results by tags (optional)")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Maximum number of results to return (default: 10, max: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of results to skip (optional)")]
    pub offset: Option<usize>,
}

// =============================================================================
// Note Tools
// =============================================================================

#[derive(Clone)]
pub struct NoteTools<D: Database> {
    db: Arc<D>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> NoteTools<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    /// Get the tool router for this handler
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    #[tool(
        description = "List notes with optional filtering by tags and note type (default: metadata only)"
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
                sort_by: None,
                sort_order: None,
            },
            tags: params.0.tags.clone(),
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

    #[tool(description = "Get a note by ID with optional content exclusion")]
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
        description = "Create a new note. Size limits: warns at 10k chars, soft max 50k chars, hard max 100k chars. For large content, split into multiple notes and link using tags (e.g., 'parent:NOTE_ID', 'related:NOTE_ID'). See docs/mcp.md for tag conventions."
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
            created_at: String::new(), // Will be set by DB
            updated_at: String::new(), // Will be set by DB
        };

        let created = self.db.notes().create(&note).await.map_err(map_db_error)?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&created).unwrap(),
        )]))
    }

    #[tool(
        description = "Update an existing note. Size limits: warns at 10k chars, soft max 50k chars, hard max 100k chars. For large content, split into multiple notes and link using tags (e.g., 'parent:NOTE_ID', 'related:NOTE_ID'). See docs/mcp.md for tag conventions."
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

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&updated).unwrap(),
        )]))
    }

    #[tool(description = "Delete a note")]
    pub async fn delete_note(
        &self,
        params: Parameters<DeleteNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .notes()
            .delete(&params.0.note_id)
            .await
            .map_err(map_db_error)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Note {} deleted successfully",
            params.0.note_id
        ))]))
    }

    #[tool(description = "Full-text search notes using FTS5")]
    pub async fn search_notes(
        &self,
        params: Parameters<SearchNotesParams>,
    ) -> Result<CallToolResult, McpError> {
        // Build query
        let query = NoteQuery {
            page: PageSort {
                limit: Some(apply_limit(params.0.limit)),
                offset: params.0.offset,
                sort_by: None,
                sort_order: None,
            },
            tags: params.0.tags.clone(),
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
