//! Shared helper functions for SQLite repositories.

use crate::db::{PageSort, SortOrder};

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
                "completed_at" => Some("completed_at"),
                _ => None,
            };
        }
    }
    None
}

/// Build ORDER BY clause from PageSort parameters.
pub fn build_order_clause(page: &PageSort, allowed_fields: &[&str], default_field: &str) -> String {
    let sort_field = page
        .sort_by
        .as_deref()
        .and_then(|f| validate_sort_field(f, allowed_fields))
        .unwrap_or(default_field);

    let order = match page.sort_order.unwrap_or(SortOrder::Asc) {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    };

    format!("ORDER BY {} {}", sort_field, order)
}

/// Build LIMIT/OFFSET clause from PageSort parameters.
/// Note: SQL requires LIMIT when using OFFSET. If offset is provided without limit,
/// we use LIMIT -1 (SQLite's "no limit" value).
pub fn build_limit_offset_clause(page: &PageSort) -> String {
    let mut clause = String::new();

    let has_offset = page.offset.is_some_and(|o| o > 0);

    if let Some(limit) = page.limit {
        clause.push_str(&format!(" LIMIT {}", limit));
    } else if has_offset {
        // SQLite requires LIMIT when using OFFSET
        // Use -1 to mean "no limit" in SQLite
        clause.push_str(" LIMIT -1");
    }

    if has_offset {
        clause.push_str(&format!(" OFFSET {}", page.offset.unwrap()));
    }

    clause
}
