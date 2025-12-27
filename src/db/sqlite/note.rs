//! SQLite NoteRepository implementation.

use std::str::FromStr;

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{DbError, DbResult, ListResult, Note, NoteQuery, NoteRepository, NoteType};

/// SQLx-backed note repository.
pub struct SqliteNoteRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> NoteRepository for SqliteNoteRepository<'a> {
    async fn create(&self, note: &Note) -> DbResult<Note> {
        // Use provided ID if not empty, otherwise generate one
        let id = if note.id.is_empty() {
            generate_entity_id()
        } else {
            note.id.clone()
        };

        // Always generate current timestamps - never use input timestamps
        let created_at = current_timestamp();
        let updated_at = created_at.clone();

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
        .bind(&id)
        .bind(&note.title)
        .bind(&note.content)
        .bind(tags_json)
        .bind(note_type_str)
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
            note_type: note.note_type.clone(),
            repo_ids: note.repo_ids.clone(),
            project_ids: note.project_ids.clone(),
            created_at,
            updated_at,
        })
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
            let tags_json: String = row.get("tags");
            let tags: Vec<String> =
                serde_json::from_str(&tags_json).map_err(|e| DbError::Database {
                    message: format!("Failed to parse tags JSON: {}", e),
                })?;

            let note_type_str: String = row.get("note_type");
            let note_type = NoteType::from_str(&note_type_str).map_err(|_| DbError::Database {
                message: format!("Invalid note_type: {}", note_type_str),
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
                note_type,
                repo_ids,
                project_ids,
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
        let allowed_fields = ["title", "created_at", "updated_at"];

        let order_clause = build_order_clause(&query.page, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(&query.page);

        // Tag filtering requires json_each join
        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());
        let mut bind_values: Vec<String> = Vec::new();

        let (sql, count_sql) = if needs_json_each {
            let tags = query.tags.as_ref().unwrap();
            let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
            bind_values.extend(tags.clone());

            (
                format!(
                    "SELECT DISTINCT n.id, n.title, n.content, n.tags, n.note_type, n.created_at, n.updated_at 
                     FROM note n, json_each(n.tags)
                     WHERE json_each.value IN ({}) {} {}",
                    placeholders.join(", "),
                    order_clause,
                    limit_clause
                ),
                format!(
                    "SELECT COUNT(DISTINCT n.id) FROM note n, json_each(n.tags) WHERE json_each.value IN ({})",
                    placeholders.join(", ")
                ),
            )
        } else {
            (
                format!(
                    "SELECT id, title, content, tags, note_type, created_at, updated_at 
                     FROM note {} {}",
                    order_clause, limit_clause
                ),
                "SELECT COUNT(*) FROM note".to_string(),
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

                let note_type_str: String = row.get("note_type");
                let note_type = NoteType::from_str(&note_type_str).unwrap_or_default();

                Note {
                    id: row.get("id"),
                    title: row.get("title"),
                    content: row.get("content"),
                    tags,
                    note_type,
                    repo_ids: vec![], // Empty by default - relationships managed separately
                    project_ids: vec![], // Empty by default - relationships managed separately
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

        // Sync repo relationships (delete old, insert new)
        sqlx::query("DELETE FROM note_repo WHERE note_id = ?")
            .bind(&note.id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        for repo_id in &note.repo_ids {
            sqlx::query("INSERT INTO note_repo (note_id, repo_id) VALUES (?, ?)")
                .bind(&note.id)
                .bind(repo_id)
                .execute(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
        }

        // Sync project relationships (delete old, insert new)
        sqlx::query("DELETE FROM project_note WHERE note_id = ?")
            .bind(&note.id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        for project_id in &note.project_ids {
            sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
                .bind(project_id)
                .bind(&note.id)
                .execute(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
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
        search_term: &str,
        query: Option<&NoteQuery>,
    ) -> DbResult<ListResult<Note>> {
        let default_query = NoteQuery::default();
        let query = query.unwrap_or(&default_query);

        let order_clause = build_order_clause(
            &query.page,
            &["title", "created_at", "updated_at"],
            "created_at",
        );
        let limit_clause = build_limit_offset_clause(&query.page);
        let search_pattern = format!("%{}%", search_term);

        // Tag filtering requires json_each join
        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());
        let mut bind_values: Vec<String> = vec![search_pattern.clone(), search_pattern.clone()];

        let (sql, count_sql) = if needs_json_each {
            let tags = query.tags.as_ref().unwrap();
            let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
            bind_values.extend(tags.clone());

            (
                format!(
                    "SELECT DISTINCT n.id, n.title, n.content, n.tags, n.note_type, n.created_at, n.updated_at 
                     FROM note n, json_each(n.tags)
                     WHERE (n.title LIKE ? OR n.content LIKE ?) AND json_each.value IN ({})
                     {} {}",
                    placeholders.join(", "),
                    order_clause,
                    limit_clause
                ),
                format!(
                    "SELECT COUNT(DISTINCT n.id) FROM note n, json_each(n.tags) WHERE (n.title LIKE ? OR n.content LIKE ?) AND json_each.value IN ({})",
                    placeholders.join(", ")
                ),
            )
        } else {
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

                let note_type_str: String = row.get("note_type");
                let note_type = NoteType::from_str(&note_type_str).unwrap_or_default();

                Note {
                    id: row.get("id"),
                    title: row.get("title"),
                    content: row.get("content"),
                    tags,
                    note_type,
                    repo_ids: vec![], // Empty by default - relationships managed separately
                    project_ids: vec![], // Empty by default - relationships managed separately
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
