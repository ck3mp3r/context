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
use sha2::Digest;
use std::sync::Arc;

use crate::api::notifier::{ChangeNotifier, UpdateMessage};
use crate::mcp::tools::anchors::{AnchorError, compute_section_etag, resolve_anchor};
use crate::mcp::tools::map_db_error;
use context_core::{HasNotes, Note, NoteQuery, NoteRepository, PageSort};

// =============================================================================
// ETag Helper
// =============================================================================

/// Compute ETag from updated_at timestamp for concurrency control.
/// Returns an 8-character hex string (truncated SHA256 hash).
fn compute_etag(updated_at: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, updated_at.as_bytes());
    hash_prefix(&hasher.finalize())
}

/// Compute the ETag of a single line range from its bounds and current text.
/// Returns an 8-character hex string (truncated SHA256 hash).
fn compute_range_etag(start: usize, end: usize, range_text: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(
        &mut hasher,
        format!("{start}\n{end}\n{range_text}").as_bytes(),
    );
    hash_prefix(&hasher.finalize())
}

/// Fold a SHA256 digest into its first 8 lowercase hex characters.
pub(crate) fn hash_prefix(digest: &[u8]) -> String {
    digest.iter().take(4).fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        write!(acc, "{:02x}", b).unwrap();
        acc
    })
}

/// Convert an anchor resolution failure into an MCP invalid-params error.
fn map_anchor_error(err: AnchorError, note_id: &str) -> McpError {
    match err {
        AnchorError::NotFound { anchor } => McpError::invalid_params(
            format!("No line matches anchor '{anchor}'."),
            Some(json!({ "note_id": note_id, "anchor": anchor })),
        ),
        AnchorError::Ambiguous { anchor, lines } => {
            let candidates: Vec<String> = lines.iter().map(usize::to_string).collect();
            McpError::invalid_params(
                format!(
                    "Anchor '{anchor}' is ambiguous: it matches lines {}.",
                    candidates.join(", ")
                ),
                Some(json!({ "note_id": note_id, "anchor": anchor, "lines": lines })),
            )
        }
    }
}

/// Return a patch anchor, treating an empty or whitespace-only string as absent.
fn patch_anchor(patch: &LinePatch) -> Option<&str> {
    patch
        .anchor
        .as_deref()
        .filter(|anchor| !anchor.trim().is_empty())
}

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
        return "lines[0]{ln,text}:".to_string();
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

/// Format one line group per range as a single TOON tabular array.
/// Each group keeps its own absolute line numbers, so disjoint ranges show
/// gaps: ranges (1,3) and (7,9) render ln 1,2,3,7,8,9.
fn format_ranges_as_toon(ranges: &[(usize, usize)], line_groups: &[String]) -> String {
    let groups: Vec<Vec<&str>> = line_groups
        .iter()
        .map(|group| group.split('\n').collect())
        .collect();
    let total: usize = groups.iter().map(Vec::len).sum();

    if total == 0 {
        return "lines[0]{ln,text}:".to_string();
    }

    let mut result = format!("lines[{total}]{{ln,text}}:");

    for ((start, _end), group) in ranges.iter().zip(groups.iter()) {
        for (idx, line) in group.iter().enumerate() {
            let line_num = start + idx;
            result.push_str(&format!("\n  {},{}", line_num, escape_toon_value(line)));
        }
    }

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
// Response Builder
// =============================================================================

/// Build the response body shared by range reads and anchor reads.
///
/// `ranges` and `etags` align 1:1. An anchor read passes `resolved` so the
/// response carries the anchor and its resolved range. Content is TOON by
/// default; `format="json"` returns `line_groups` instead.
fn range_read_response(
    note_id: &str,
    resolved: Option<(&str, (usize, usize))>,
    ranges: &[(usize, usize)],
    line_contents: &[String],
    etags: &[String],
    etag_scope: &str,
    format: Option<&str>,
) -> serde_json::Value {
    let ranges_json: Vec<serde_json::Value> = ranges
        .iter()
        .map(|(start, end)| json!({"start": start, "end": end}))
        .collect();

    let etags_json: Vec<serde_json::Value> = ranges
        .iter()
        .zip(etags.iter())
        .map(|((start, end), etag)| json!({"start": start, "end": end, "etag": etag}))
        .collect();

    let mut response = json!({
        "note_id": note_id,
        "ranges": ranges_json,
        "etags": etags_json,
        "etag_scope": etag_scope,
    });

    if let Some((anchor, (start, end))) = resolved {
        response["anchor"] = json!(anchor);
        response["resolved_range"] = json!({"start": start, "end": end});
    }

    if format == Some("json") {
        response["line_groups"] = json!(line_contents);
    } else {
        response["content"] = json!(format_ranges_as_toon(ranges, line_contents));
    }

    response
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

/// A patch to apply to a note. Each patch targets exactly one location, either
/// the line range [start, end] or the range resolved from `anchor`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(inline)]
pub struct LinePatch {
    #[schemars(
        description = "Start line number (1-indexed, inclusive). Required with 'end'; omit when using 'anchor'."
    )]
    pub start: Option<usize>,
    #[schemars(
        description = "End line number (1-indexed, inclusive). Required with 'start'; omit when using 'anchor'."
    )]
    pub end: Option<usize>,
    #[schemars(description = "Replacement text for the given line range")]
    pub content: String,
    #[schemars(
        description = "Range etag from note_read or section etag from an anchor read; validates this target against current content"
    )]
    pub etag: Option<String>,
    #[schemars(
        description = "Anchor that locates the patch target. Heading mode (e.g. '## Foo') covers the section; any other string covers the single line containing it. Mutually exclusive with 'start'/'end'."
    )]
    pub anchor: Option<String>,
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
        description = "Line ranges to fetch. Omit for full content. An empty array is an error. Sorted & validated for overlaps."
    )]
    pub ranges: Option<Vec<LineRange>>,
    #[schemars(
        description = "Anchor that locates one section. Heading mode: 1-6 '#' plus a space and the heading title, e.g. '## Foo'; the title matches case-insensitively at the same heading level and the section runs from the heading to the line before the next heading of the same or higher level. Text mode: any other string matches the unique line containing it (case-sensitive) and the section is that line only. Zero matches return 'not_found'; multiple matches return 'ambiguous' with the candidate line numbers. An empty or whitespace-only anchor is rejected. Mutually exclusive with ranges."
    )]
    pub anchor: Option<String>,
    #[schemars(
        description = "Format: 'toon' (default, with line numbers) or 'json' (plain content)"
    )]
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateNoteParams {
    #[schemars(description = "Note title")]
    pub title: String,
    #[schemars(
        description = "Note content (Markdown). Keep under 10k chars. For larger content, use parent:NOTE_ID tag."
    )]
    pub content: String,
    #[schemars(
        description = "Tags. Use parent:NOTE_ID (continuation), related:NOTE_ID (reference), session (persistent)."
    )]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "Parent note ID to make this note a subnote. Omit this parameter for a top-level note."
    )]
    #[serde(
        default,
        deserialize_with = "crate::common::serde::deserialize_option_string"
    )]
    pub parent_id: Option<String>,
    #[schemars(description = "Manual ordering index (optional)")]
    pub idx: Option<i32>,
    #[schemars(description = "Repository IDs to link (optional)")]
    pub repo_ids: Option<Vec<String>>,
    #[schemars(description = "Project IDs to link (REQUIRED for session notes)")]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteNoteParams {
    #[schemars(description = "Note ID")]
    pub note_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RestoreNoteParams {
    #[schemars(description = "Note ID (8-character hex)")]
    pub note_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditNoteParams {
    #[schemars(description = "Note ID")]
    pub note_id: String,
    #[schemars(
        description = "Whole-note ETag from note_read. Required for metadata-only edits; when every patch carries a range etag it may be omitted."
    )]
    pub etag: Option<String>,
    #[schemars(description = "Note title (optional)")]
    pub title: Option<String>,
    #[schemars(description = "Tags. Use parent:NOTE_ID, related:NOTE_ID. Replaces all existing.")]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "Parent note ID to make this note a subnote. Omit this parameter to leave unchanged. Pass empty string \"\" to remove an existing parent."
    )]
    #[serde(
        default,
        deserialize_with = "crate::common::serde::deserialize_option_string"
    )]
    pub parent_id: Option<String>,
    #[schemars(description = "Manual ordering index (optional)")]
    #[serde(default, deserialize_with = "crate::common::serde::double_option")]
    pub idx: Option<Option<i32>>,
    #[schemars(description = "Repository IDs to link (optional)")]
    pub repo_ids: Option<Vec<String>>,
    #[schemars(description = "Project IDs to link (REQUIRED for session notes)")]
    pub project_ids: Option<Vec<String>>,
    #[schemars(
        description = "Patches. Each targets either the line range [start, end] or an 'anchor'; anchors resolve server-side against the current content. Validated before any write, applied in reverse order."
    )]
    pub patches: Vec<LinePatch>,
}

#[derive(Clone)]
pub struct NoteTools<D: HasNotes> {
    db: Arc<D>,
    notifier: ChangeNotifier,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: HasNotes + 'static> NoteTools<D> {
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
        description = "List notes. Query for FTS search. Default: metadata only. Filter by tags/project_id/parent_id/note_type. Limit: 10 (max 20)."
    )]
    pub async fn note_list(
        &self,
        params: Parameters<ListNotesParams>,
    ) -> Result<CallToolResult, McpError> {
        // Build query
        let query = NoteQuery {
            page: PageSort {
                limit: params.0.limit,
                offset: params.0.offset,
                sort_by: params.0.sort.clone(),
                sort_order: match params.0.order.as_deref() {
                    Some("desc") => Some(context_core::SortOrder::Desc),
                    Some("asc") => Some(context_core::SortOrder::Asc),
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
        } else {
            self.db.notes().list_metadata_only(Some(&query)).await
        }
        .map_err(map_db_error)?;

        let mut items = result.items;
        for note in &mut items {
            note.content = String::new();
        }

        let response = json!({
            "items": items,
            "total": result.total,
            "limit": result.limit,
            "offset": result.offset,
        });

        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&response).unwrap(),
        )]))
    }

    #[tool(
        description = "Create note (Markdown). Keep under 10k chars. For larger: split & link with parent:NOTE_ID tag."
    )]
    pub async fn note_create(
        &self,
        params: Parameters<CreateNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        let note = Note {
            id: String::new(), // Will be generated by DB
            title: params.0.title.clone(),
            content: params.0.content.clone(),
            tags: params.0.tags.clone().unwrap_or_default(),
            parent_id: params
                .0
                .parent_id
                .as_ref()
                .and_then(|s| if s.is_empty() { None } else { Some(s.clone()) }),
            idx: params.0.idx,
            repo_ids: params.0.repo_ids.clone().unwrap_or_default(),
            project_ids: params.0.project_ids.clone().unwrap_or_default(),
            subnote_count: None,
            created_at: None, // Will be set by DB
            updated_at: None, // Will be set by DB
            deleted_at: None,
        };

        let created = self.db.notes().create(&note).await.map_err(map_db_error)?;

        // Broadcast NoteCreated notification
        self.notifier.notify(UpdateMessage::NoteCreated {
            note_id: created.id.clone(),
        });

        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&created).unwrap(),
        )]))
    }

    #[tool(description = "Delete note permanently. Consider archiving with tags instead.")]
    pub async fn note_delete(
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

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "Note {} deleted successfully",
            params.0.note_id
        ))]))
    }

    #[tool(description = "Restore a soft-deleted note")]
    pub async fn note_restore(
        &self,
        params: Parameters<RestoreNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .notes()
            .restore(&params.0.note_id)
            .await
            .map_err(map_db_error)?;

        self.notifier.notify(UpdateMessage::NoteUpdated {
            note_id: params.0.note_id.clone(),
        });

        let content = json!({
            "success": true,
            "message": format!("Note {} restored successfully", params.0.note_id)
        });

        let text = serde_json::to_string_pretty(&content).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[tool(
        description = "Read note. Full read returns the whole-note etag; a range read returns one etag per range (etag_scope='range'); an anchor read resolves a heading like '## Foo' or a unique text line to one section (etag_scope='section'). Content is TOON (line numbers) unless format='json'. Ranges: omit=full; an empty array is an error."
    )]
    pub async fn note_read(
        &self,
        params: Parameters<ReadNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        // Handle params semantics:
        // - anchor: Some(_) and ranges: Some(_): invalid_params error
        // - anchor: Some(_): resolve the anchor and return its single section
        // - ranges: None: return full note with content (default)
        // - ranges: Some([]): invalid_params error (an empty array names no ranges)
        // - ranges: Some([ranges]): return specific line ranges

        if let Some(anchor) = params.0.anchor.as_deref() {
            if params.0.ranges.is_some() {
                return Err(McpError::invalid_params(
                    "Provide either 'anchor' or 'ranges', not both.",
                    Some(json!({
                        "note_id": params.0.note_id,
                        "anchor": anchor
                    })),
                ));
            }

            if anchor.trim().is_empty() {
                return Err(McpError::invalid_params(
                    "Empty anchor. Provide a heading like '## Foo' or a non-empty text line.",
                    Some(json!({
                        "note_id": params.0.note_id,
                        "anchor": anchor
                    })),
                ));
            }

            let note = self.db.notes().get(&params.0.note_id).await.map_err(|e| {
                McpError::resource_not_found(
                    "note_not_found",
                    Some(serde_json::json!({"error": e.to_string()})),
                )
            })?;

            let (start, end) = resolve_anchor(&note.content, anchor)
                .map_err(|err| map_anchor_error(err, &params.0.note_id))?;

            let lines: Vec<&str> = note.content.lines().collect();
            let Some(section_lines) = lines.get(start - 1..end) else {
                return Err(McpError::internal_error(
                    "anchor_out_of_bounds",
                    Some(json!({
                        "note_id": params.0.note_id,
                        "start": start,
                        "end": end
                    })),
                ));
            };
            let section_text = section_lines.join("\n");
            let section_etag = compute_section_etag(&section_text);

            let response = range_read_response(
                &params.0.note_id,
                Some((anchor, (start, end))),
                &[(start, end)],
                std::slice::from_ref(&section_text),
                std::slice::from_ref(&section_etag),
                "section",
                params.0.format.as_deref(),
            );

            return Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&response).unwrap(),
            )]));
        }

        match &params.0.ranges {
            // No ranges specified: return full note with content
            None => {
                let mut note = self.db.notes().get(&params.0.note_id).await.map_err(|e| {
                    McpError::resource_not_found(
                        "note_not_found",
                        Some(serde_json::json!({"error": e.to_string()})),
                    )
                })?;

                // Compute etag from updated_at
                let etag = compute_etag(note.updated_at.as_ref().unwrap_or(&String::new()));

                // Apply TOON formatting by default (can opt-out with format="json")
                if params.0.format.as_deref() != Some("json") {
                    note.content = format_as_toon(&note.content, 1);
                }

                // Create response with etag
                let mut note_json = serde_json::to_value(&note).unwrap();
                note_json["etag"] = serde_json::Value::String(etag);

                Ok(CallToolResult::success(vec![ContentBlock::text(
                    serde_json::to_string_pretty(&note_json).unwrap(),
                )]))
            }

            // Empty array: an empty ranges array names no ranges, so reject it
            Some(ranges) if ranges.is_empty() => Err(McpError::invalid_params(
                "Empty ranges array. Omit ranges for a full read or specify one or more line ranges.",
                Some(serde_json::json!({
                    "note_id": params.0.note_id
                })),
            )),

            // Specific line ranges: return line groups with per-range etags
            Some(ranges) => {
                // Sort ascending so the response ranges, line groups, and etags align
                let mut sorted: Vec<(usize, usize)> =
                    ranges.iter().map(|r| (r.start, r.end)).collect();
                sorted.sort_by_key(|r| r.0);

                let line_contents = self
                    .db
                    .notes()
                    .get_line_ranges(&params.0.note_id, &sorted)
                    .await
                    .map_err(map_db_error)?;

                let etags: Vec<String> = sorted
                    .iter()
                    .zip(line_contents.iter())
                    .map(|((start, end), text)| compute_range_etag(*start, *end, text))
                    .collect();

                let response = range_read_response(
                    &params.0.note_id,
                    None,
                    &sorted,
                    &line_contents,
                    &etags,
                    "range",
                    params.0.format.as_deref(),
                );

                Ok(CallToolResult::success(vec![ContentBlock::text(
                    serde_json::to_string_pretty(&response).unwrap(),
                )]))
            }
        }
    }

    #[tool(
        description = "Edit note. Requires a whole-note etag from note_read, or one range etag (range read) or section etag (anchor read) per patch. Each patch targets a line range or an anchor. Updates metadata and/or applies patches."
    )]
    pub async fn note_edit(
        &self,
        params: Parameters<EditNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        // Get existing note to validate etag
        let current_note = self
            .db
            .notes()
            .get(&params.0.note_id)
            .await
            .map_err(map_db_error)?;

        // Compute current etag from updated_at
        let current_etag = compute_etag(current_note.updated_at.as_ref().unwrap_or(&String::new()));

        let provided_etag = params.0.etag.as_deref().unwrap_or("");
        let has_patches = !params.0.patches.is_empty();

        // Resolve every patch target against the current content before any etag
        // validation. Exactly one target shape is allowed per patch.
        let current_lines: Vec<&str> = current_note.content.lines().collect();
        let mut resolved: Vec<((usize, usize), String)> =
            Vec::with_capacity(params.0.patches.len());

        for (index, patch) in params.0.patches.iter().enumerate() {
            let range = match (patch.start, patch.end, patch_anchor(patch)) {
                (Some(start), Some(end), None) => (start, end),
                (None, None, Some(anchor)) => resolve_anchor(&current_note.content, anchor)
                    .map_err(|err| map_anchor_error(err, &params.0.note_id))?,
                _ => {
                    return Err(McpError::invalid_params(
                        format!(
                            "Patch {index} must target either a line range ('start' and 'end') or an 'anchor'; provide exactly one."
                        ),
                        Some(json!({
                            "note_id": params.0.note_id,
                            "patch_index": index
                        })),
                    ));
                }
            };

            resolved.push((range, patch.content.clone()));
        }

        // A non-empty whole-note etag must match the current note
        if !provided_etag.is_empty() && current_etag != provided_etag {
            return Err(McpError::invalid_params(
                "Note has been modified since last read. Please re-read the note before editing.",
                Some(serde_json::json!({
                    "note_id": params.0.note_id,
                    "provided_etag": provided_etag
                })),
            ));
        }

        // Metadata-only edits have no range to guard, so they need the whole-note etag
        if !has_patches && provided_etag.is_empty() {
            return Err(McpError::invalid_params(
                "Editing note metadata requires the whole-note etag from note_read.",
                Some(serde_json::json!({
                    "note_id": params.0.note_id
                })),
            ));
        }

        // Validate every patch etag against the content fetched before any write.
        // A line patch binds its range etag; an anchor patch binds its section etag.
        for (patch, ((start, end), _content)) in params.0.patches.iter().zip(resolved.iter()) {
            let (start, end) = (*start, *end);

            if start < 1 || end < start || end > current_lines.len() {
                return Err(McpError::invalid_params(
                    format!(
                        "Invalid line range ({start}, {end}) for a note with {} lines.",
                        current_lines.len()
                    ),
                    Some(serde_json::json!({
                        "note_id": params.0.note_id,
                        "start": start,
                        "end": end
                    })),
                ));
            }

            let patch_etag = patch.etag.as_deref().unwrap_or("");

            // Without a target etag the whole-note etag must cover this patch
            let missing_etag = patch_etag.is_empty();

            if missing_etag && provided_etag.is_empty() {
                return Err(McpError::invalid_params(
                    format!(
                        "Patch for lines ({start}, {end}) has no etag. Provide the range etag from note_read or the whole-note etag."
                    ),
                    Some(serde_json::json!({
                        "note_id": params.0.note_id,
                        "start": start,
                        "end": end
                    })),
                ));
            }

            if missing_etag {
                continue;
            }

            let range_text = current_lines[start - 1..end].join("\n");
            let uses_anchor = patch_anchor(patch).is_some();

            let (current_target_etag, message) = if uses_anchor {
                (
                    compute_section_etag(&range_text),
                    format!(
                        "Section ({start}, {end}) has been modified since last read. Please re-read the section before editing."
                    ),
                )
            } else {
                (
                    compute_range_etag(start, end, &range_text),
                    format!(
                        "Range ({start}, {end}) has been modified since last read. Please re-read the ranges before editing."
                    ),
                )
            };

            if current_target_etag != patch_etag {
                return Err(McpError::invalid_params(
                    message,
                    Some(serde_json::json!({
                        "note_id": params.0.note_id,
                        "start": start,
                        "end": end,
                        "provided_etag": patch_etag
                    })),
                ));
            }
        }

        // Get existing note for editing
        let mut note = current_note;

        // Update metadata fields (same as update_note)
        if let Some(title) = &params.0.title {
            note.title = title.clone();
        }
        if let Some(tags) = &params.0.tags {
            note.tags = tags.clone();
        }
        if let Some(parent_id) = &params.0.parent_id {
            if parent_id.is_empty() {
                note.parent_id = None;
            } else {
                note.parent_id = Some(parent_id.clone());
            }
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

        // Apply the resolved line-range patches to content if provided
        if !resolved.is_empty() {
            self.db
                .notes()
                .apply_line_patches(&params.0.note_id, &resolved)
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

        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&updated).unwrap(),
        )]))
    }
}

#[cfg(test)]
#[path = "../../../test/mcp/tools/notes.rs"]
mod notes_test;
