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

/// Sanitize and transform an FTS5 search query to prevent syntax errors.
///
/// This function:
/// 1. Strips FTS5-dangerous special characters (except quotes, underscore, whitespace)
/// 2. Handles unbalanced quotes by removing them
/// 3. Returns None for empty/whitespace-only queries
/// 4. Detects advanced search features (Boolean operators, phrases)
/// 5. Adds prefix matching (*) for simple queries
///
/// Returns Some(sanitized_query) or None if query is empty after sanitization.
pub fn sanitize_fts5_query(search_term: &str) -> Option<String> {
    // Strip FTS5-dangerous special characters
    // Keep: alphanumeric, underscore, quotes, whitespace, non-ASCII (for unicode)
    let cleaned = search_term
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric()
                || c == '_'
                || c == '"'
                || c.is_whitespace()
                || (c as u32) > 127
            {
                c
            } else {
                ' '
            }
        })
        .collect::<String>();

    // Handle unbalanced quotes - if odd number, remove all quotes
    let quote_count = cleaned.chars().filter(|c| *c == '"').count();
    let cleaned = if quote_count % 2 == 0 {
        cleaned
    } else {
        cleaned.replace('"', "")
    };

    // Return None for empty/whitespace-only queries
    if cleaned.trim().is_empty() {
        return None;
    }

    // Detect advanced search features
    let has_boolean =
        cleaned.contains(" AND ") || cleaned.contains(" OR ") || cleaned.contains(" NOT ");
    let has_phrase = cleaned.contains('"');

    // Apply query transformation
    let result = if has_boolean || has_phrase {
        // Advanced mode - preserve query as-is
        cleaned
    } else {
        // Simple mode - add prefix matching to each term
        cleaned
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|term| format!("{}*", term))
            .collect::<Vec<_>>()
            .join(" ")
    };

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_fts5_query_simple() {
        assert_eq!(sanitize_fts5_query("rust"), Some("rust*".to_string()));
    }

    #[test]
    fn test_sanitize_fts5_query_multiple_terms() {
        assert_eq!(
            sanitize_fts5_query("rust async"),
            Some("rust* async*".to_string())
        );
    }

    #[test]
    fn test_sanitize_fts5_query_boolean_operators() {
        assert_eq!(
            sanitize_fts5_query("rust AND async"),
            Some("rust AND async".to_string())
        );
        assert_eq!(
            sanitize_fts5_query("backend OR frontend"),
            Some("backend OR frontend".to_string())
        );
        assert_eq!(
            sanitize_fts5_query("code NOT deprecated"),
            Some("code NOT deprecated".to_string())
        );
    }

    #[test]
    fn test_sanitize_fts5_query_phrase() {
        assert_eq!(
            sanitize_fts5_query("\"exact match\""),
            Some("\"exact match\"".to_string())
        );
    }

    #[test]
    fn test_sanitize_fts5_query_empty() {
        assert_eq!(sanitize_fts5_query(""), None);
        assert_eq!(sanitize_fts5_query("   "), None);
        assert_eq!(sanitize_fts5_query("\t\n"), None);
    }

    #[test]
    fn test_sanitize_fts5_query_special_chars() {
        // Special FTS5 chars should be stripped
        assert_eq!(
            sanitize_fts5_query("hello@world#test"),
            Some("hello* world* test*".to_string())
        );
    }

    #[test]
    fn test_sanitize_fts5_query_unbalanced_quotes() {
        // Odd number of quotes - should remove all quotes
        assert_eq!(
            sanitize_fts5_query("hello \"world"),
            Some("hello* world*".to_string())
        );
    }

    #[test]
    fn test_sanitize_fts5_query_unicode() {
        // Unicode characters should be preserved
        assert_eq!(
            sanitize_fts5_query("Rust über"),
            Some("Rust* über*".to_string())
        );
    }
}
