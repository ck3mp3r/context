//! SQLite NoteRepository implementation.

use sqlx::{Row, SqlitePool};

use super::helpers::build_limit_offset_clause;
use crate::db::models::{NOTE_HARD_MAX, NOTE_SOFT_MAX, NOTE_WARN_SIZE};
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{DbError, DbResult, ListResult, Note, NoteQuery, NoteRepository};

/// SQLx-backed note repository.
pub struct SqliteNoteRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

/// Validates note content size.
///
/// Returns Ok(warning_message) if note is large but acceptable (WARN_SIZE to HARD_MAX).
/// Returns Ok(None) if note is within normal size.
/// Returns Err if note exceeds HARD_MAX.
fn validate_note_size(content: &str) -> DbResult<Option<String>> {
    let size = content.len();

    if size > NOTE_HARD_MAX {
        return Err(DbError::Validation {
            message: format!(
                "Note content exceeds maximum size of {} characters (got {} characters). \
                 Consider splitting into multiple notes using parent:NOTE_ID tags.",
                NOTE_HARD_MAX, size
            ),
        });
    }

    if size > NOTE_WARN_SIZE {
        let warning = if size > NOTE_SOFT_MAX {
            format!(
                "Note is very large ({} characters, soft max {}). \
                 Consider splitting for better performance.",
                size, NOTE_SOFT_MAX
            )
        } else {
            format!(
                "Note is large ({} characters, warning threshold {}). \
                 Consider splitting into related notes using parent:NOTE_ID tags.",
                size, NOTE_WARN_SIZE
            )
        };
        Ok(Some(warning))
    } else {
        Ok(None)
    }
}

impl<'a> NoteRepository for SqliteNoteRepository<'a> {
    async fn create(&self, note: &Note) -> DbResult<Note> {
        // Validate content size
        validate_note_size(&note.content)?;

        // Use provided ID if not empty, otherwise generate one
        let id = if note.id.is_empty() {
            generate_entity_id()
        } else {
            note.id.clone()
        };

        // Use provided timestamps or generate if None/empty
        let created_at = note
            .created_at
            .clone()
            .filter(|s| !s.is_empty()) // Treat empty string as None (backward compat)
            .unwrap_or_else(current_timestamp);
        let updated_at = note
            .updated_at
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(current_timestamp);

        let tags_json = serde_json::to_string(&note.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;

        sqlx::query(
            r#"
            INSERT INTO note (id, title, content, tags, parent_id, idx, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&note.title)
        .bind(&note.content)
        .bind(tags_json)
        .bind(&note.parent_id)
        .bind(note.idx)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Insert repo relationships
        for repo_id in &note.repo_ids {
            sqlx::query("INSERT INTO note_repo (note_id, repo_id) VALUES (?, ?)")
                .bind(&id)
                .bind(repo_id)
                .execute(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
        }

        // Insert project relationships
        for project_id in &note.project_ids {
            sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
                .bind(project_id)
                .bind(&id)
                .execute(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
        }

        Ok(Note {
            id,
            title: note.title.clone(),
            content: note.content.clone(),
            tags: note.tags.clone(),
            parent_id: note.parent_id.clone(),
            idx: note.idx,
            repo_ids: note.repo_ids.clone(),
            project_ids: note.project_ids.clone(),
            subnote_count: None, // Not computed for single note get
            created_at: Some(created_at),
            updated_at: Some(updated_at),
        })
    }

    async fn get(&self, id: &str) -> DbResult<Note> {
        let row = sqlx::query(
            "SELECT id, title, content, tags, parent_id, idx, created_at, updated_at FROM note WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        if let Some(row) = row {
            let tags_json: String = row.get("tags");
            let tags: Vec<String> =
                serde_json::from_str(&tags_json).map_err(|e| DbError::Database {
                    message: format!("Failed to parse tags JSON: {}", e),
                })?;

            // Get repo relationships
            let repo_ids: Vec<String> =
                sqlx::query_scalar("SELECT repo_id FROM note_repo WHERE note_id = ?")
                    .bind(id)
                    .fetch_all(self.pool)
                    .await
                    .map_err(|e| DbError::Database {
                        message: e.to_string(),
                    })?;

            // Get project relationships
            let project_ids: Vec<String> =
                sqlx::query_scalar("SELECT project_id FROM project_note WHERE note_id = ?")
                    .bind(id)
                    .fetch_all(self.pool)
                    .await
                    .map_err(|e| DbError::Database {
                        message: e.to_string(),
                    })?;

            Ok(Note {
                id: row.get("id"),
                title: row.get("title"),
                content: row.get("content"),
                tags,
                parent_id: row.get("parent_id"),
                idx: row.get("idx"),
                repo_ids,
                project_ids,
                subnote_count: None, // Not computed for single note get
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
        } else {
            Err(DbError::NotFound {
                entity_type: "Note".to_string(),
                id: id.to_string(),
            })
        }
    }

    async fn get_metadata_only(&self, id: &str) -> DbResult<Note> {
        let row = sqlx::query(
            "SELECT id, title, tags, parent_id, idx, created_at, updated_at FROM note WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        if let Some(row) = row {
            let tags_json: String = row.get("tags");
            let tags: Vec<String> =
                serde_json::from_str(&tags_json).map_err(|e| DbError::Database {
                    message: format!("Failed to parse tags JSON: {}", e),
                })?;

            // Get repo relationships
            let repo_ids: Vec<String> =
                sqlx::query_scalar("SELECT repo_id FROM note_repo WHERE note_id = ?")
                    .bind(id)
                    .fetch_all(self.pool)
                    .await
                    .map_err(|e| DbError::Database {
                        message: e.to_string(),
                    })?;

            // Get project relationships
            let project_ids: Vec<String> =
                sqlx::query_scalar("SELECT project_id FROM project_note WHERE note_id = ?")
                    .bind(id)
                    .fetch_all(self.pool)
                    .await
                    .map_err(|e| DbError::Database {
                        message: e.to_string(),
                    })?;

            Ok(Note {
                id: row.get("id"),
                title: row.get("title"),
                content: String::new(), // Empty content for metadata-only
                tags,
                parent_id: row.get("parent_id"),
                idx: row.get("idx"),
                repo_ids,
                project_ids,
                subnote_count: None, // Not computed for single note get
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
        } else {
            Err(DbError::NotFound {
                entity_type: "Note".to_string(),
                id: id.to_string(),
            })
        }
    }

    async fn list(&self, query: Option<&NoteQuery>) -> DbResult<ListResult<Note>> {
        let default_query = NoteQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = [
            "title",
            "created_at",
            "updated_at",
            "last_activity_at",
            "idx",
        ];

        // Determine which JOINs are needed
        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());
        let needs_project_join = query.project_id.is_some();

        // Build query conditionally based on what filters are needed
        let mut bind_values: Vec<String> = Vec::new();
        let mut where_conditions: Vec<String> = Vec::new();

        // Check if we need last_activity_at computed column for parent notes
        let needs_activity_column = query.note_type.as_deref() == Some("note");

        // Decide on table alias usage
        let (select_cols, from_clause, order_field_prefix) = if needs_json_each
            || needs_project_join
        {
            // Need aliases when doing JOINs
            let mut from = "FROM note n".to_string();

            if needs_project_join {
                from.push_str("\nINNER JOIN project_note pn ON n.id = pn.note_id");
                where_conditions.push("pn.project_id = ?".to_string());
                bind_values.push(query.project_id.as_ref().unwrap().clone());
            }

            if needs_json_each {
                from.push_str(", json_each(n.tags)");
                let tags = query.tags.as_ref().unwrap();
                let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
                where_conditions.push(format!("json_each.value IN ({})", placeholders.join(", ")));
                bind_values.extend(tags.clone());
            }

            let select = if needs_activity_column {
                "DISTINCT n.id, n.title, n.content, n.tags, n.parent_id, n.idx, n.created_at, n.updated_at, \
                 COALESCE((SELECT MAX(updated_at) FROM note WHERE parent_id = n.id), n.updated_at) AS last_activity_at, \
                 (SELECT COUNT(*) FROM note WHERE parent_id = n.id) AS subnote_count"
            } else {
                "DISTINCT n.id, n.title, n.content, n.tags, n.parent_id, n.idx, n.created_at, n.updated_at"
            };

            (select, from, "n.")
        } else {
            // No joins, simple query
            let select = if needs_activity_column {
                // Explicitly reference outer table in subquery using table name
                "note.id, note.title, note.content, note.tags, note.parent_id, note.idx, note.created_at, note.updated_at, \
                 COALESCE((SELECT MAX(updated_at) FROM note AS child WHERE child.parent_id = note.id), note.updated_at) AS last_activity_at, \
                 (SELECT COUNT(*) FROM note AS child WHERE child.parent_id = note.id) AS subnote_count"
            } else {
                "id, title, content, tags, parent_id, idx, created_at, updated_at"
            };

            (
                select,
                "FROM note".to_string(),
                if needs_activity_column { "note." } else { "" },
            )
        };

        // Add parent_id filter if specified (after we know the table prefix)
        if let Some(parent_id) = &query.parent_id {
            where_conditions.push(format!("{}parent_id = ?", order_field_prefix));
            bind_values.push(parent_id.clone());
        }

        // Filter by note type: "note" (parent_id IS NULL) or "subnote" (parent_id IS NOT NULL)
        if let Some(note_type) = &query.note_type {
            match note_type.as_str() {
                "note" => where_conditions.push(format!("{}parent_id IS NULL", order_field_prefix)),
                "subnote" => {
                    where_conditions.push(format!("{}parent_id IS NOT NULL", order_field_prefix))
                }
                _ => {} // Ignore invalid values
            }
        }

        // Build WHERE clause
        let where_clause = if !where_conditions.is_empty() {
            format!("WHERE {}", where_conditions.join(" AND "))
        } else {
            String::new()
        };

        // Build ORDER BY with proper prefixes
        // Special handling: when querying by parent_id, default to ordering by idx
        let order_clause = if query.parent_id.is_some() && query.page.sort_by.is_none() {
            // Default order for subnotes: idx ASC (lowest first), then updated_at DESC (latest first)
            format!(
                "ORDER BY {}idx ASC, {}updated_at DESC",
                order_field_prefix, order_field_prefix
            )
        } else if needs_activity_column && query.page.sort_by.is_none() {
            // Default order for parent notes: most recently active first
            "ORDER BY last_activity_at DESC".to_string()
        } else {
            let sort_field = query
                .page
                .sort_by
                .as_deref()
                .filter(|f| allowed_fields.contains(f))
                .unwrap_or("created_at");
            let sort_order = match query.page.sort_order.unwrap_or(crate::db::SortOrder::Asc) {
                crate::db::SortOrder::Asc => "ASC",
                crate::db::SortOrder::Desc => "DESC",
            };

            // Handle last_activity_at sort field
            if sort_field == "last_activity_at" {
                format!("ORDER BY last_activity_at {}", sort_order)
            } else {
                format!(
                    "ORDER BY {}{} {}",
                    order_field_prefix, sort_field, sort_order
                )
            }
        };

        let limit_clause = build_limit_offset_clause(&query.page);

        // Build final SQL
        let sql = format!(
            "SELECT {} {} {} {} {}",
            select_cols, from_clause, where_clause, order_clause, limit_clause
        );

        let count_sql = if needs_json_each || needs_project_join {
            format!(
                "SELECT COUNT(DISTINCT n.id) {} {}",
                from_clause, where_clause
            )
        } else {
            format!("SELECT COUNT(*) FROM note {}", where_clause)
        };

        // Get paginated results
        let mut query_builder = sqlx::query(&sql);
        for value in &bind_values {
            query_builder = query_builder.bind(value);
        }

        let rows = query_builder
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let items: Vec<Note> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

                // Try to get subnote_count if it exists in the result set
                let subnote_count = row.try_get::<i32, _>("subnote_count").ok();

                Note {
                    id: row.get("id"),
                    title: row.get("title"),
                    content: row.get("content"),
                    tags,
                    parent_id: row.get("parent_id"),
                    idx: row.get("idx"),
                    repo_ids: vec![], // Empty by default - relationships managed separately
                    project_ids: vec![], // Empty by default - relationships managed separately
                    subnote_count,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();

        // Get total count
        let mut count_query = sqlx::query_scalar(&count_sql);
        for value in &bind_values {
            count_query = count_query.bind(value);
        }

        let total: i64 = count_query
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(ListResult {
            items,
            total: total as usize,
            limit: query.page.limit,
            offset: query.page.offset.unwrap_or(0),
        })
    }

    async fn list_metadata_only(&self, query: Option<&NoteQuery>) -> DbResult<ListResult<Note>> {
        let default_query = NoteQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = [
            "title",
            "created_at",
            "updated_at",
            "last_activity_at",
            "idx",
        ];

        // Check if we need last_activity_at computed column for parent notes
        let needs_activity_column = query.note_type.as_deref() == Some("note");

        // Tag filtering requires json_each join
        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());
        let mut bind_values: Vec<String> = Vec::new();
        let mut where_conditions: Vec<String> = Vec::new();

        // Determine table prefix based on whether we need JOINs
        let order_field_prefix = if needs_json_each {
            "n."
        } else if needs_activity_column {
            "note."
        } else {
            ""
        };

        // Add parent_id filter if specified
        if let Some(parent_id) = &query.parent_id {
            where_conditions.push(format!("{}parent_id = ?", order_field_prefix));
            bind_values.push(parent_id.clone());
        }

        // Filter by note type: "note" (parent_id IS NULL) or "subnote" (parent_id IS NOT NULL)
        if let Some(note_type) = &query.note_type {
            match note_type.as_str() {
                "note" => where_conditions.push(format!("{}parent_id IS NULL", order_field_prefix)),
                "subnote" => {
                    where_conditions.push(format!("{}parent_id IS NOT NULL", order_field_prefix))
                }
                _ => {} // Ignore invalid values
            }
        }

        let where_clause = if !where_conditions.is_empty() {
            format!("WHERE {}", where_conditions.join(" AND "))
        } else {
            String::new()
        };

        // Build ORDER BY - special handling for different query types
        let order_clause = if query.parent_id.is_some() && query.page.sort_by.is_none() {
            // Default order for subnotes: idx ASC (lowest first), then updated_at DESC (latest first)
            format!(
                "ORDER BY {}idx ASC, {}updated_at DESC",
                order_field_prefix, order_field_prefix
            )
        } else if needs_activity_column && query.page.sort_by.is_none() {
            // Default order for parent notes: most recently active first
            "ORDER BY last_activity_at DESC".to_string()
        } else {
            let sort_field = query
                .page
                .sort_by
                .as_deref()
                .filter(|f| allowed_fields.contains(f))
                .unwrap_or("created_at");
            let sort_order = match query.page.sort_order.unwrap_or(crate::db::SortOrder::Asc) {
                crate::db::SortOrder::Asc => "ASC",
                crate::db::SortOrder::Desc => "DESC",
            };

            // Handle last_activity_at sort field
            if sort_field == "last_activity_at" {
                format!("ORDER BY last_activity_at {}", sort_order)
            } else {
                format!(
                    "ORDER BY {}{} {}",
                    order_field_prefix, sort_field, sort_order
                )
            }
        };

        let limit_clause = build_limit_offset_clause(&query.page);

        let (sql, count_sql) = if needs_json_each {
            let tags = query.tags.as_ref().unwrap();
            let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
            bind_values.extend(tags.clone());

            let select_cols = if needs_activity_column {
                "DISTINCT n.id, n.title, n.tags, n.parent_id, n.idx, n.created_at, n.updated_at, \
                 (SELECT COUNT(*) FROM note WHERE parent_id = n.id) AS subnote_count, \
                 COALESCE((SELECT MAX(updated_at) FROM note WHERE parent_id = n.id), n.updated_at) AS last_activity_at"
            } else {
                "DISTINCT n.id, n.title, n.tags, n.parent_id, n.idx, n.created_at, n.updated_at"
            };

            (
                format!(
                    "SELECT {} 
                     FROM note n, json_each(n.tags)
                     WHERE json_each.value IN ({}) {} {}",
                    select_cols,
                    placeholders.join(", "),
                    order_clause,
                    limit_clause
                ),
                format!(
                    "SELECT COUNT(DISTINCT n.id) FROM note n, json_each(n.tags) WHERE json_each.value IN ({})",
                    placeholders.join(", ")
                ),
            )
        } else if needs_activity_column {
            (
                format!(
                    "SELECT note.id, note.title, note.tags, note.parent_id, note.idx, note.created_at, note.updated_at, \
                     (SELECT COUNT(*) FROM note AS child WHERE child.parent_id = note.id) AS subnote_count, \
                     COALESCE((SELECT MAX(updated_at) FROM note AS child WHERE child.parent_id = note.id), note.updated_at) AS last_activity_at 
                     FROM note {} {} {}",
                    where_clause, order_clause, limit_clause
                ),
                format!("SELECT COUNT(*) FROM note {}", where_clause),
            )
        } else {
            (
                format!(
                    "SELECT id, title, tags, parent_id, idx, created_at, updated_at 
                     FROM note {} {} {}",
                    where_clause, order_clause, limit_clause
                ),
                format!("SELECT COUNT(*) FROM note {}", where_clause),
            )
        };

        // Get paginated results
        let mut query_builder = sqlx::query(&sql);
        for value in &bind_values {
            query_builder = query_builder.bind(value);
        }

        let rows = query_builder
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let items: Vec<Note> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

                // Try to get subnote_count if it exists in the result set
                let subnote_count = row.try_get::<i32, _>("subnote_count").ok();

                Note {
                    id: row.get("id"),
                    title: row.get("title"),
                    content: String::new(), // metadata_only doesn't include content
                    tags,
                    parent_id: row.get("parent_id"),
                    idx: row.get("idx"),
                    repo_ids: vec![], // Empty by default - relationships managed separately
                    project_ids: vec![], // Empty by default - relationships managed separately
                    subnote_count,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();

        // Get total count
        let mut count_query = sqlx::query_scalar(&count_sql);
        for value in &bind_values {
            count_query = count_query.bind(value);
        }

        let total: i64 = count_query
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(ListResult {
            items,
            total: total as usize,
            limit: query.page.limit,
            offset: query.page.offset.unwrap_or(0),
        })
    }

    async fn update(&self, note: &Note) -> DbResult<()> {
        // Validate content size
        validate_note_size(&note.content)?;

        // Use transaction for atomicity
        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let tags_json = serde_json::to_string(&note.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;

        // Use provided timestamp or generate if None/empty
        let updated_at = note
            .updated_at
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(current_timestamp);

        let result = sqlx::query(
            r#"
            UPDATE note 
            SET title = ?, content = ?, tags = ?, parent_id = ?, idx = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&note.title)
        .bind(&note.content)
        .bind(tags_json)
        .bind(&note.parent_id)
        .bind(note.idx)
        .bind(&updated_at)
        .bind(&note.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Note".to_string(),
                id: note.id.clone(),
            });
        }

        // Sync repo relationships (delete old, insert new)
        sqlx::query("DELETE FROM note_repo WHERE note_id = ?")
            .bind(&note.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        for repo_id in &note.repo_ids {
            sqlx::query("INSERT INTO note_repo (note_id, repo_id) VALUES (?, ?)")
                .bind(&note.id)
                .bind(repo_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
        }

        // Sync project relationships (delete old, insert new)
        sqlx::query("DELETE FROM project_note WHERE note_id = ?")
            .bind(&note.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        for project_id in &note.project_ids {
            sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
                .bind(project_id)
                .bind(&note.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
        }

        tx.commit().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM note WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Note".to_string(),
                id: id.to_string(),
            });
        }

        Ok(())
    }

    async fn search(
        &self,
        search_term: &str,
        query: Option<&NoteQuery>,
    ) -> DbResult<ListResult<Note>> {
        let default_query = NoteQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["title", "created_at", "updated_at", "last_activity_at"];

        // Determine which JOINs are needed
        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());
        let needs_project_join = query.project_id.is_some();

        // Check if we need last_activity_at computed column for parent notes
        let needs_activity_column = query.note_type.as_deref() == Some("note");

        // Sanitize FTS5 search query to prevent syntax errors
        // FTS5 has strict bareword requirements - only allows: A-Z, a-z, 0-9, _, non-ASCII
        // Strategy: Strip dangerous chars, balance quotes, preserve Boolean ops, add prefix matching
        let fts_query = {
            // Step 1: Strip FTS5-dangerous special characters using allowlist
            // FTS5 barewords ONLY allow: A-Z, a-z, 0-9, _, non-ASCII (>127), quotes, spaces
            // Replace all other characters with spaces to prevent syntax errors
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

            // Step 2: Handle unbalanced quotes
            // FTS5 requires balanced quotes for phrase searches
            let quote_count = cleaned.chars().filter(|c| *c == '"').count();
            let cleaned = if quote_count % 2 == 0 {
                cleaned // Balanced - preserve phrase search capability
            } else {
                cleaned.replace('"', "") // Unbalanced - strip all quotes
            };

            // Step 3: Handle empty/whitespace-only queries
            if cleaned.trim().is_empty() {
                return Ok(ListResult {
                    items: vec![],
                    total: 0,
                    limit: query.page.limit,
                    offset: query.page.offset.unwrap_or(0),
                });
            }

            // Step 4: Detect advanced search features
            let has_boolean =
                cleaned.contains(" AND ") || cleaned.contains(" OR ") || cleaned.contains(" NOT ");
            let has_phrase = cleaned.contains('"');

            // Step 5: Apply query transformation
            if has_boolean || has_phrase {
                // Advanced mode - preserve operators and phrases
                cleaned
            } else {
                // Simple mode - add prefix matching for fuzzy search
                cleaned
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .map(|term| format!("{}*", term))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        };

        let mut bind_values: Vec<String> = vec![fts_query];
        let mut where_conditions: Vec<String> = Vec::new();

        // Use FTS5 for search - join note_fts to note table
        let (select_cols, from_clause, order_field_prefix) = if needs_json_each
            || needs_project_join
        {
            // Need aliases when doing JOINs
            let mut from =
                "FROM note n INNER JOIN note_fts ON n.rowid = note_fts.rowid".to_string();

            if needs_project_join {
                from.push_str("\nINNER JOIN project_note pn ON n.id = pn.note_id");
                where_conditions.push("pn.project_id = ?".to_string());
                bind_values.push(query.project_id.as_ref().unwrap().clone());
            }

            if needs_json_each {
                from.push_str(", json_each(n.tags)");
                let tags = query.tags.as_ref().unwrap();
                let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
                where_conditions.push(format!("json_each.value IN ({})", placeholders.join(", ")));
                bind_values.extend(tags.clone());
            }

            let select = if needs_activity_column {
                "DISTINCT n.id, n.title, n.content, n.tags, n.parent_id, n.idx, n.created_at, n.updated_at, \
                 COALESCE((SELECT MAX(updated_at) FROM note WHERE parent_id = n.id), n.updated_at) AS last_activity_at"
            } else {
                "DISTINCT n.id, n.title, n.content, n.tags, n.parent_id, n.idx, n.created_at, n.updated_at"
            };

            (select, from, "n.")
        } else {
            // No filters, simple FTS5 join - use explicit table prefix
            let select = if needs_activity_column {
                "note.id, note.title, note.content, note.tags, note.parent_id, note.idx, note.created_at, note.updated_at, \
                 COALESCE((SELECT MAX(updated_at) FROM note AS child WHERE child.parent_id = note.id), note.updated_at) AS last_activity_at"
            } else {
                "note.id, note.title, note.content, note.tags, note.parent_id, note.idx, note.created_at, note.updated_at"
            };

            (
                select,
                "FROM note INNER JOIN note_fts ON note.rowid = note_fts.rowid".to_string(),
                "note.",
            )
        };

        // Add parent_id filter if specified (after we know the table prefix)
        if let Some(parent_id) = &query.parent_id {
            where_conditions.push(format!("{}parent_id = ?", order_field_prefix));
            bind_values.push(parent_id.clone());
        }

        // Filter by note type: "note" (parent_id IS NULL) or "subnote" (parent_id IS NOT NULL)
        if let Some(note_type) = &query.note_type {
            match note_type.as_str() {
                "note" => where_conditions.push(format!("{}parent_id IS NULL", order_field_prefix)),
                "subnote" => {
                    where_conditions.push(format!("{}parent_id IS NOT NULL", order_field_prefix))
                }
                _ => {} // Ignore invalid values
            }
        }

        // FTS5 MATCH condition - searches across title, content, and tags
        where_conditions.insert(0, "note_fts MATCH ?".to_string());
        let where_clause = format!("WHERE {}", where_conditions.join(" AND "));

        // Build ORDER BY with proper prefixes
        let order_clause = if needs_activity_column && query.page.sort_by.is_none() {
            // Default order for parent notes: most recently active first
            "ORDER BY last_activity_at DESC".to_string()
        } else {
            let sort_field = query
                .page
                .sort_by
                .as_deref()
                .filter(|f| allowed_fields.contains(f))
                .unwrap_or("created_at");
            let sort_order = match query.page.sort_order.unwrap_or(crate::db::SortOrder::Asc) {
                crate::db::SortOrder::Asc => "ASC",
                crate::db::SortOrder::Desc => "DESC",
            };

            // Handle last_activity_at sort field
            if sort_field == "last_activity_at" {
                format!("ORDER BY last_activity_at {}", sort_order)
            } else {
                format!(
                    "ORDER BY {}{} {}",
                    order_field_prefix, sort_field, sort_order
                )
            }
        };

        let limit_clause = build_limit_offset_clause(&query.page);

        // Build final SQL
        let sql = format!(
            "SELECT {} {} {} {} {}",
            select_cols, from_clause, where_clause, order_clause, limit_clause
        );

        let count_sql = if needs_json_each || needs_project_join {
            format!(
                "SELECT COUNT(DISTINCT n.id) {} {}",
                from_clause, where_clause
            )
        } else {
            format!("SELECT COUNT(*) {} {}", from_clause, where_clause)
        };

        // Get paginated results
        let mut query_builder = sqlx::query(&sql);
        for value in &bind_values {
            query_builder = query_builder.bind(value);
        }

        let rows = query_builder
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let items: Vec<Note> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

                // Try to get subnote_count if it exists in the result set
                let subnote_count = row.try_get::<i32, _>("subnote_count").ok();

                Note {
                    id: row.get("id"),
                    title: row.get("title"),
                    content: row.get("content"),
                    tags,
                    parent_id: row.get("parent_id"),
                    idx: row.get("idx"),
                    repo_ids: vec![], // Empty by default - relationships managed separately
                    project_ids: vec![], // Empty by default - relationships managed separately
                    subnote_count,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();

        // Get total count
        let mut count_query = sqlx::query_scalar(&count_sql);
        for value in &bind_values {
            count_query = count_query.bind(value);
        }

        let total: i64 = count_query
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(ListResult {
            items,
            total: total as usize,
            limit: query.page.limit,
            offset: query.page.offset.unwrap_or(0),
        })
    }
}
