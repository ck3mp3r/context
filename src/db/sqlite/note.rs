//! SQLite NoteRepository implementation.

use std::str::FromStr;

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause, build_tag_filter};
use crate::db::{DbError, DbResult, ListQuery, ListResult, Note, NoteRepository, NoteType};

/// SQLx-backed note repository.
pub struct SqliteNoteRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> NoteRepository for SqliteNoteRepository<'a> {
    async fn create(&self, note: &Note) -> DbResult<()> {
        let tags_json = serde_json::to_string(&note.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;

        let note_type_str = note.note_type.to_string();

        sqlx::query(
            r#"
            INSERT INTO note (id, title, content, tags, note_type, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&note.id)
        .bind(&note.title)
        .bind(&note.content)
        .bind(tags_json)
        .bind(note_type_str)
        .bind(&note.created_at)
        .bind(&note.updated_at)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(())
    }

    async fn get(&self, id: &str) -> DbResult<Note> {
        let row = sqlx::query(
            "SELECT id, title, content, tags, note_type, created_at, updated_at FROM note WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        if let Some(row) = row {
            // Parse tags JSON
            let tags_json: String = row.get("tags");
            let tags: Vec<String> =
                serde_json::from_str(&tags_json).map_err(|e| DbError::Database {
                    message: format!("Failed to parse tags JSON: {}", e),
                })?;

            // Parse note_type
            let note_type_str: String = row.get("note_type");
            let note_type = NoteType::from_str(&note_type_str).map_err(|_| DbError::Database {
                message: format!("Invalid note_type: {}", note_type_str),
            })?;

            Ok(Note {
                id: row.get("id"),
                title: row.get("title"),
                content: row.get("content"),
                tags,
                note_type,
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

    async fn list(&self, query: Option<&ListQuery>) -> DbResult<ListResult<Note>> {
        let default_query = ListQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["title", "created_at", "updated_at"];

        // Build query components
        let order_clause = build_order_clause(query, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(query);
        let tag_filter = build_tag_filter(query);

        // Build query with optional tag filtering
        let (sql, count_sql) = if tag_filter.where_clause.is_empty() {
            // No tag filtering
            (
                format!(
                    "SELECT id, title, content, tags, note_type, created_at, updated_at 
                     FROM note {} {}",
                    order_clause, limit_clause
                ),
                "SELECT COUNT(*) FROM note".to_string(),
            )
        } else {
            // With tag filtering using json_each
            (
                format!(
                    "SELECT DISTINCT n.id, n.title, n.content, n.tags, n.note_type, n.created_at, n.updated_at 
                     FROM note n, json_each(n.tags)
                     WHERE {} {} {}",
                    tag_filter.where_clause, order_clause, limit_clause
                ),
                format!(
                    "SELECT COUNT(DISTINCT n.id) FROM note n, json_each(n.tags) WHERE {}",
                    tag_filter.where_clause
                ),
            )
        };

        // Get paginated results
        let mut query_builder = sqlx::query(&sql);
        for tag in &tag_filter.bind_values {
            query_builder = query_builder.bind(tag);
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
                // Parse tags JSON
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

                // Parse note_type
                let note_type_str: String = row.get("note_type");
                let note_type = NoteType::from_str(&note_type_str).unwrap_or_default();

                Note {
                    id: row.get("id"),
                    title: row.get("title"),
                    content: row.get("content"),
                    tags,
                    note_type,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();

        // Get total count
        let mut count_query = sqlx::query_scalar(&count_sql);
        for tag in &tag_filter.bind_values {
            count_query = count_query.bind(tag);
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
            limit: query.limit,
            offset: query.offset.unwrap_or(0),
        })
    }

    async fn update(&self, note: &Note) -> DbResult<()> {
        let tags_json = serde_json::to_string(&note.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;

        let note_type_str = note.note_type.to_string();

        let result = sqlx::query(
            r#"
            UPDATE note 
            SET title = ?, content = ?, tags = ?, note_type = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&note.title)
        .bind(&note.content)
        .bind(tags_json)
        .bind(note_type_str)
        .bind(&note.updated_at)
        .bind(&note.id)
        .execute(self.pool)
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
        query: &str,
        pagination: Option<&ListQuery>,
    ) -> DbResult<ListResult<Note>> {
        let default_pagination = ListQuery::default();
        let pagination = pagination.unwrap_or(&default_pagination);

        // Build query components
        let order_clause = build_order_clause(
            pagination,
            &["title", "created_at", "updated_at"],
            "created_at",
        );
        let limit_clause = build_limit_offset_clause(pagination);
        let tag_filter = build_tag_filter(pagination);
        let search_pattern = format!("%{}%", query);

        // Build query with search and optional tag filtering
        let (sql, count_sql) = if tag_filter.where_clause.is_empty() {
            // No tag filtering - simple search
            (
                format!(
                    "SELECT id, title, content, tags, note_type, created_at, updated_at 
                     FROM note 
                     WHERE (title LIKE ? OR content LIKE ?)
                     {} {}",
                    order_clause, limit_clause
                ),
                "SELECT COUNT(*) FROM note WHERE (title LIKE ? OR content LIKE ?)".to_string(),
            )
        } else {
            // With tag filtering using json_each
            (
                format!(
                    "SELECT DISTINCT n.id, n.title, n.content, n.tags, n.note_type, n.created_at, n.updated_at 
                     FROM note n, json_each(n.tags)
                     WHERE (n.title LIKE ? OR n.content LIKE ?) AND {}
                     {} {}",
                    tag_filter.where_clause, order_clause, limit_clause
                ),
                format!(
                    "SELECT COUNT(DISTINCT n.id) FROM note n, json_each(n.tags) WHERE (n.title LIKE ? OR n.content LIKE ?) AND {}",
                    tag_filter.where_clause
                ),
            )
        };

        // Get paginated results - bind search pattern first, then tag values
        let mut query_builder = sqlx::query(&sql);
        query_builder = query_builder.bind(&search_pattern).bind(&search_pattern);
        for tag in &tag_filter.bind_values {
            query_builder = query_builder.bind(tag);
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
                // Parse tags JSON
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

                // Parse note_type
                let note_type_str: String = row.get("note_type");
                let note_type = NoteType::from_str(&note_type_str).unwrap_or_default();

                Note {
                    id: row.get("id"),
                    title: row.get("title"),
                    content: row.get("content"),
                    tags,
                    note_type,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();

        // Get total count for search - bind search pattern first, then tag values
        let mut count_query = sqlx::query_scalar(&count_sql);
        count_query = count_query.bind(&search_pattern).bind(&search_pattern);
        for tag in &tag_filter.bind_values {
            count_query = count_query.bind(tag);
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
            limit: pagination.limit,
            offset: pagination.offset.unwrap_or(0),
        })
    }
}
