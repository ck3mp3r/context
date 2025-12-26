//! Shared helper functions for SQLite repositories.

use crate::db::{ListQuery, SortOrder};

/// Tag filter result containing the WHERE clause fragment and bind values.
pub struct TagFilter {
    /// SQL WHERE clause fragment (e.g., "(json_each.value IN (?, ?))").
    /// Empty string if no tags to filter.
    pub where_clause: String,
    /// Values to bind for the placeholders.
    pub bind_values: Vec<String>,
}

/// Build a tag filter clause for JSON array stored tags.
///
/// SQLite stores tags as JSON arrays like `["rust", "programming"]`.
/// This uses json_each to match if ANY tag in the filter matches ANY tag in the record.
///
/// Returns the WHERE clause fragment and bind values. The caller must join
/// using `json_each(tags)` in the FROM clause when there are tags to filter.
pub fn build_tag_filter(query: &ListQuery) -> TagFilter {
    match &query.tags {
        Some(tags) if !tags.is_empty() => {
            let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
            TagFilter {
                where_clause: format!("json_each.value IN ({})", placeholders.join(", ")),
                bind_values: tags.clone(),
            }
        }
        _ => TagFilter {
            where_clause: String::new(),
            bind_values: vec![],
        },
    }
}

/// Validate and map a sort field to the actual column name.
/// Returns None for invalid fields (falls back to default).
pub fn validate_sort_field(field: &str, allowed: &[&str]) -> Option<&'static str> {
    for &allowed_field in allowed {
        if field == allowed_field {
            // Return static str to avoid lifetime issues
            return match field {
                "title" => Some("title"),
                "name" => Some("name"),
                "content" => Some("content"),
                "status" => Some("status"),
                "priority" => Some("priority"),
                "note_type" => Some("note_type"),
                "remote" => Some("remote"),
                "path" => Some("path"),
                "created_at" => Some("created_at"),
                "updated_at" => Some("updated_at"),
                _ => None,
            };
        }
    }
    None
}

/// Build ORDER BY clause from query parameters.
pub fn build_order_clause(
    query: &ListQuery,
    allowed_fields: &[&str],
    default_field: &str,
) -> String {
    let sort_field = query
        .sort_by
        .as_deref()
        .and_then(|f| validate_sort_field(f, allowed_fields))
        .unwrap_or(default_field);

    let order = match query.sort_order.unwrap_or(SortOrder::Asc) {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    };

    format!("ORDER BY {} {}", sort_field, order)
}

/// Build LIMIT/OFFSET clause from query parameters.
pub fn build_limit_offset_clause(query: &ListQuery) -> String {
    let mut clause = String::new();
    if let Some(limit) = query.limit {
        clause.push_str(&format!(" LIMIT {}", limit));
    }
    if let Some(offset) = query.offset
        && offset > 0
    {
        clause.push_str(&format!(" OFFSET {}", offset));
    }
    clause
}
