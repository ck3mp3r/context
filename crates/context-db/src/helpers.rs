//! Shared helper functions for SQLite repositories.

use context_core::{DbError, DbResult, PageSort, SortOrder};

/// Soft-delete a single entity row by setting `deleted_at` to the current time.
///
/// When `bump_updated_at` is true the row's `updated_at` is set to the current
/// time as well, so the tombstone flows through incremental sync. Tables
/// without an `updated_at` column (currently only `repo`) pass false.
///
/// Returns `Ok(true)` when this call soft-deleted the row, `Ok(false)` when the
/// row exists but is already soft-deleted (idempotent no-op), and
/// `Err(DbError::NotFound)` when no row with `id` exists.
pub async fn soft_delete_row(
    conn: &mut sqlx::SqliteConnection,
    table: &str,
    entity_type: &str,
    id: &str,
    bump_updated_at: bool,
) -> DbResult<bool> {
    let set_clause = if bump_updated_at {
        "deleted_at = datetime('now'), updated_at = datetime('now')"
    } else {
        "deleted_at = datetime('now')"
    };
    let sql = format!(
        "UPDATE {} SET {} WHERE id = ? AND deleted_at IS NULL",
        table, set_clause
    );

    let result = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

    if result.rows_affected() > 0 {
        return Ok(true);
    }

    let exists_sql = format!("SELECT id FROM {} WHERE id = ?", table);
    let exists: Option<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(exists_sql.as_str()))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

    if exists.is_none() {
        return Err(DbError::NotFound {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
        });
    }

    Ok(false)
}

/// Read the `deleted_at` tombstone marker for a row. Test-only helper.
#[cfg(test)]
pub async fn read_deleted_at(pool: &sqlx::SqlitePool, table: &str, id: &str) -> Option<String> {
    use sqlx::Row;

    let sql = format!("SELECT deleted_at FROM {} WHERE id = ?", table);
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("row should exist");

    row.get("deleted_at")
}

/// Read the `updated_at` timestamp for a row. Test-only helper.
#[cfg(test)]
pub async fn read_updated_at(pool: &sqlx::SqlitePool, table: &str, id: &str) -> Option<String> {
    use sqlx::Row;

    let sql = format!("SELECT updated_at FROM {} WHERE id = ?", table);
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("row should exist");

    row.get("updated_at")
}

/// Count rows in `table` matching `where_clause` with a single `?` bound to `id`.
/// Test-only helper.
#[cfg(test)]
pub async fn count_rows(pool: &sqlx::SqlitePool, table: &str, where_clause: &str, id: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {} WHERE {}", table, where_clause);
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("count should succeed")
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
    // Keep: alphanumeric, underscore, quotes, whitespace, non-ASCII (for unicode),
    //       and FTS5 boolean operators: (, ), *
    let cleaned = search_term
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric()
                || c == '_'
                || c == '"'
                || c == '('
                || c == ')'
                || c == '*'
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
    // Also return None if the only remaining content is FTS5 operators without search terms
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed == "*" || trimmed == "(" || trimmed == ")" {
        return None;
    }

    // Normalize boolean operators to uppercase for consistent detection
    let normalized = cleaned
        .split_whitespace()
        .map(|word| {
            let upper = word.to_uppercase();
            match upper.as_str() {
                "AND" | "OR" | "NOT" => upper,
                _ => word.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    // Detect advanced search features
    let has_boolean =
        normalized.contains(" AND ") || normalized.contains(" OR ") || normalized.contains(" NOT ");
    let has_phrase = normalized.contains('"');

    // Validate that the query has actual search terms, not just operators
    let has_search_terms = normalized
        .split_whitespace()
        .any(|w| w != "AND" && w != "OR" && w != "NOT" && !w.is_empty());

    // Apply query transformation
    let result = if has_boolean || has_phrase {
        if has_search_terms {
            // Advanced mode - preserve query as-is
            normalized
        } else {
            // Only operators without search terms - return empty results
            return None;
        }
    } else {
        // Simple mode - strip parentheses and add prefix matching to each term
        let stripped = normalized.replace(['(', ')'], "");
        stripped
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|term| {
                if term.ends_with('*') {
                    term.to_string()
                } else {
                    format!("{}*", term)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };

    Some(result)
}
