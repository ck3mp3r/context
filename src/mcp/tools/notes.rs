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
use crate::db::{Database, Note, NoteQuery, NoteRepository, PageSort};
use crate::mcp::tools::map_db_error;

// =============================================================================
// TOON Formatting Helper
// =============================================================================

/// Format content lines as TOON tabular array with line numbers.
/// Format: lines[N]{ln,text}:
///   1,First line
///   2,"Second line, with comma"
fn format_as_toon(content: &str, start_line: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let count = lines.len();

    if count == 0 {
        return format!("lines[0]{{ln,text}}:");
    }

    let mut result = format!("lines[{}]{{ln,text}}:\n", count);

    for (idx, line) in lines.iter().enumerate() {
        let line_num = start_line + idx;
        let escaped_line = escape_toon_value(line);
        result.push_str(&format!("  {},{}\n", line_num, escaped_line));
    }

    // Remove trailing newline
    result.pop();
    result
}

/// Escape a value for TOON tabular format (comma-delimited).
/// Quotes the value if it contains comma, quote, newline, or starts with quote.
fn escape_toon_value(value: &str) -> String {
    let needs_quoting = value.contains(',')
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r')
        || value.starts_with('"');

    if needs_quoting {
        // Quote and escape internal quotes by doubling them (CSV-style)
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

// =============================================================================
// Parameter Structs
// =============================================================================

/// A line range for reading note content. Lines are 1-indexed.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(inline)]
pub struct LineRange {
    #[schemars(description = "Start line number (1-indexed, inclusive)")]
    pub start: usize,
    #[schemars(description = "End line number (1-indexed, inclusive)")]
    pub end: usize,
}

/// A patch to apply to a note: replaces lines [start, end] with new content.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(inline)]
pub struct LinePatch {
    #[schemars(description = "Start line number (1-indexed, inclusive)")]
    pub start: usize,
    #[schemars(description = "End line number (1-indexed, inclusive)")]
    pub end: usize,
    #[schemars(description = "Replacement text for the given line range")]
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListNotesParams {
    #[schemars(
        description = "FTS5 search query (optional). If provided, performs full-text search. Examples: 'rust AND async' (Boolean), '\"exact phrase\"' (phrase match), 'term*' (prefix), 'NOT deprecated' (exclude), 'api AND (error OR bug)' (complex)"
    )]
    pub query: Option<String>,
    #[schemars(
        description = "Filter by tags. Use reference tags to find linked notes: ['parent:NOTE_ID'], ['related:NOTE_ID']"
    )]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Filter by project ID")]
    pub project_id: Option<String>,
    #[schemars(description = "Filter by parent note ID to list subnotes")]
    pub parent_id: Option<String>,
    #[schemars(
        description = "Filter by note type: 'note' (parent notes only) or 'subnote' (subnotes only). Omit to return both parent notes and subnotes (default)."
    )]
    pub note_type: Option<String>,
    #[schemars(description = "Maximum number of items to return (default: 10, max: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of items to skip")]
    pub offset: Option<usize>,
    #[schemars(
        description = "Include note content in response (default: false for lighter list responses). Set to true to retrieve full content."
    )]
    pub include_content: Option<bool>,
    #[schemars(
        description = "Field to sort by (title, created_at, updated_at, last_activity_at). Default: created_at"
    )]
    pub sort: Option<String>,
    #[schemars(description = "Sort order (asc, desc). Default: asc")]
    pub order: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadNoteParams {
    #[schemars(description = "Note ID")]
    pub note_id: String,
    #[schemars(
        description = "Optional array of line ranges to fetch. Omit for full note content, empty array [] for metadata only, or specify ranges for specific lines. Ranges will be sorted and validated for overlaps."
    )]
    pub ranges: Option<Vec<LineRange>>,
    #[schemars(
        description = "Output format: 'json' (default, returns plain content) or 'toon' (returns TOON tabular format with explicit line numbers for easy patching)"
    )]
    pub format: Option<String>,
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
    #[schemars(description = "Parent note ID for hierarchical notes (optional)")]
    pub parent_id: Option<String>,
    #[schemars(description = "Index for manual ordering (lower values first, optional)")]
    pub idx: Option<i32>,
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
pub struct EditNoteParams {
    #[schemars(description = "Note ID")]
    pub note_id: String,
    #[schemars(description = "Note title (optional)")]
    pub title: Option<String>,
    #[schemars(
        description = "Tags (optional). Use 'parent:NOTE_ID' for continuations, 'related:NOTE_ID' for references. Replaces all existing tags when provided."
    )]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "Parent note ID for hierarchical notes (optional). Use empty string \"\" or null to remove parent."
    )]
    #[serde(
        default,
        deserialize_with = "crate::serde_utils::double_option_string_or_empty"
    )]
    pub parent_id: Option<Option<String>>,
    #[schemars(description = "Index for manual ordering (optional)")]
    #[serde(default, deserialize_with = "crate::serde_utils::double_option")]
    pub idx: Option<Option<i32>>,
    #[schemars(
        description = "Repository IDs to link (optional). Associate with relevant repos for context."
    )]
    pub repo_ids: Option<Vec<String>>,
    #[schemars(
        description = "Project IDs to link (RECOMMENDED). Attach to relevant project for organization and discoverability. REQUIRED for session notes - always link session notes to their project(s)."
    )]
    pub project_ids: Option<Vec<String>>,
    #[schemars(
        description = "Array of patches to apply. Each patch replaces lines [start, end] with new content. Patches will be sorted, validated for overlaps, and applied in reverse order to maintain accurate line numbers."
    )]
    pub patches: Vec<LinePatch>,
}

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
        description = "List notes with optional full-text search. Provide 'query' parameter to search, omit to list all. Default excludes content (metadata only) - use include_content=true for full notes. Filter by tags, project_id, parent_id, or note_type. Sort by title, created_at, updated_at, or last_activity_at. Limit default: 10, max: 20."
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
            parent_id: params.0.parent_id.clone(),
            note_type: params.0.note_type.clone(),
        };

        // If query is provided, perform FTS search
        let result = if let Some(q) = &params.0.query {
            self.db.notes().search(q, Some(&query)).await
        } else if include_content {
            self.db.notes().list(Some(&query)).await
        } else {
            self.db.notes().list_metadata_only(Some(&query)).await
        }
        .map_err(map_db_error)?;

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

    #[tool(
        description = "Create a new note (Markdown supported). IMPORTANT: Keep under 10k chars (~2.5k tokens) to avoid context overflow. For larger content, split into multiple notes and link with tags: 'parent:NOTE_ID' (continuation), 'related:NOTE_ID' (reference). Link to projects/repos via project_ids/repo_ids."
    )]
    pub async fn create_note(
        &self,
        params: Parameters<CreateNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        let note = Note {
            id: String::new(), // Will be generated by DB
            title: params.0.title.clone(),
            content: params.0.content.clone(),
            tags: params.0.tags.clone().unwrap_or_default(),
            parent_id: params.0.parent_id.clone(),
            idx: params.0.idx,
            repo_ids: params.0.repo_ids.clone().unwrap_or_default(),
            project_ids: params.0.project_ids.clone().unwrap_or_default(),
            subnote_count: None,
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
        description = "Read a note. Control content via ranges parameter: omit for full note with content, empty array [] for metadata only, or specify ranges like [{start: 1, end: 3}, {start: 7, end: 9}] for specific lines. Ranges are automatically sorted and validated for overlaps. Use format='toon' to get content with explicit line numbers (lines[N]{ln,text}: format) - essential for accurate line-based patching with c5t_edit_note."
    )]
    pub async fn read_note(
        &self,
        params: Parameters<ReadNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        // Handle ranges parameter semantics:
        // - None: return full note with content (default)
        // - Some([]): return metadata only (no content)
        // - Some([ranges]): return specific line ranges

        match &params.0.ranges {
            // No ranges specified: return full note with content
            None => {
                let mut note = self.db.notes().get(&params.0.note_id).await.map_err(|e| {
                    McpError::resource_not_found(
                        "note_not_found",
                        Some(serde_json::json!({"error": e.to_string()})),
                    )
                })?;

                // Apply TOON formatting if requested
                if params.0.format.as_deref() == Some("toon") {
                    note.content = format_as_toon(&note.content, 1);
                }

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&note).unwrap(),
                )]))
            }

            // Empty array: return metadata only (no content)
            Some(ranges) if ranges.is_empty() => {
                let note = self
                    .db
                    .notes()
                    .get_metadata_only(&params.0.note_id)
                    .await
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

            // Specific line ranges: return line groups
            Some(ranges) => {
                let ranges_tuples: Vec<(usize, usize)> =
                    ranges.iter().map(|r| (r.start, r.end)).collect();
                let line_contents = self
                    .db
                    .notes()
                    .get_line_ranges(&params.0.note_id, &ranges_tuples)
                    .await
                    .map_err(map_db_error)?;

                // Apply TOON formatting if requested
                if params.0.format.as_deref() == Some("toon") {
                    // Combine all lines into a single content string
                    let combined_content = line_contents.join("\n");

                    // Get start line number from first range
                    let start_line = ranges.first().map(|r| r.start).unwrap_or(1);

                    let formatted_content = format_as_toon(&combined_content, start_line);

                    let response = json!({
                        "note_id": params.0.note_id,
                        "ranges": ranges,
                        "content": formatted_content,
                    });

                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&response).unwrap(),
                    )]))
                } else {
                    // Return original format with line_groups
                    let response = json!({
                        "note_id": params.0.note_id,
                        "ranges": ranges,
                        "line_groups": line_contents,
                    });

                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&response).unwrap(),
                    )]))
                }
            }
        }
    }

    #[tool(
        description = "Edit a note - update metadata and/or apply line-range patches to content. All fields optional. Patches are automatically sorted, validated for overlaps, and applied in reverse order to maintain accurate line numbers. For full content replacement, use a single patch covering all lines."
    )]
    pub async fn edit_note(
        &self,
        params: Parameters<EditNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        // Get existing note
        let mut note = self
            .db
            .notes()
            .get(&params.0.note_id)
            .await
            .map_err(map_db_error)?;

        // Update metadata fields (same as update_note)
        if let Some(title) = &params.0.title {
            note.title = title.clone();
        }
        if let Some(tags) = &params.0.tags {
            note.tags = tags.clone();
        }
        if let Some(parent_id) = &params.0.parent_id {
            note.parent_id = parent_id.clone();
        }
        if let Some(idx) = &params.0.idx {
            note.idx = *idx;
        }
        if let Some(repo_ids) = &params.0.repo_ids {
            note.repo_ids = repo_ids.clone();
        }
        if let Some(project_ids) = &params.0.project_ids {
            note.project_ids = project_ids.clone();
        }

        // Apply line-range patches to content if provided
        if !params.0.patches.is_empty() {
            let patches_tuples: Vec<((usize, usize), String)> = params
                .0
                .patches
                .iter()
                .map(|p| ((p.start, p.end), p.content.clone()))
                .collect();
            self.db
                .notes()
                .apply_line_patches(&params.0.note_id, &patches_tuples)
                .await
                .map_err(map_db_error)?;

            // Fetch note again after patches to get updated content, but preserve metadata changes
            let patched_note = self
                .db
                .notes()
                .get(&params.0.note_id)
                .await
                .map_err(map_db_error)?;

            // Only update the content field, keep our metadata changes
            note.content = patched_note.content;
        }

        // Clear updated_at to ensure proper timestamp refresh (same as update_note)
        note.updated_at = None;

        // Update the note with all changes
        self.db.notes().update(&note).await.map_err(map_db_error)?;

        // Fetch updated note to get auto-set updated_at (same as update_note)
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
}
