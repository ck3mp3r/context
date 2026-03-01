//! SQLite ProjectRepository implementation.

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{
    DbError, DbResult, ListResult, Project, ProjectQuery, ProjectRepository, SortOrder,
};

/// SQLx-backed project repository.
pub struct SqliteProjectRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

fn validate_project(project: &Project) -> DbResult<()> {
    let mut errors = Vec::new();

    // Validate title (required, not empty)
    if project.title.trim().is_empty() {
        errors.push("Project title cannot be empty".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(DbError::Validation {
            message: errors.join("; "),
        })
    }
}

impl<'a> ProjectRepository for SqliteProjectRepository<'a> {
    async fn create(&self, project: &Project) -> DbResult<Project> {
        // Validate project
        validate_project(project)?;

        // Use provided ID if not empty, otherwise generate one
        let id = if project.id.is_empty() {
            generate_entity_id()
        } else {
            project.id.clone()
        };

        // Use provided timestamps or generate if None
        let created_at = project.created_at.clone().unwrap_or_else(current_timestamp);
        let updated_at = project
            .updated_at
            .clone()
            .unwrap_or_else(|| created_at.clone());

        let tags_json = serde_json::to_string(&project.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;

        let external_refs_json =
            serde_json::to_string(&project.external_refs).map_err(|e| DbError::Database {
                message: format!("Failed to serialize external_refs: {}", e),
            })?;

        sqlx::query("INSERT INTO project (id, title, description, tags, external_refs, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&project.title)
            .bind(&project.description)
            .bind(&tags_json)
            .bind(&external_refs_json)
            .bind(&created_at)
            .bind(&updated_at)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(Project {
            id,
            title: project.title.clone(),
            description: project.description.clone(),
            tags: project.tags.clone(),
            external_refs: project.external_refs.clone(),
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some(created_at),
            updated_at: Some(updated_at),
        })
    }

    async fn get(&self, id: &str) -> DbResult<Project> {
        let row = sqlx::query(
            "SELECT id, title, description, tags, external_refs, created_at, updated_at FROM project WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let row = row.ok_or(DbError::NotFound {
            entity_type: "Project".to_string(),
            id: id.to_string(),
        })?;

        let tags_json: String = row.get("tags");
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

        let external_refs_json: String = row.get("external_refs");
        let external_refs: Vec<String> =
            serde_json::from_str(&external_refs_json).unwrap_or_default();

        // Get repo relationships
        let repo_ids: Vec<String> =
            sqlx::query_scalar("SELECT repo_id FROM project_repo WHERE project_id = ?")
                .bind(id)
                .fetch_all(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        // Get task list relationships (now 1:N from task_list.project_id)
        let task_list_ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM task_list WHERE project_id = ?")
                .bind(id)
                .fetch_all(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        // Get note relationships
        let note_ids: Vec<String> =
            sqlx::query_scalar("SELECT note_id FROM project_note WHERE project_id = ?")
                .bind(id)
                .fetch_all(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        Ok(Project {
            id: row.get("id"),
            title: row.get("title"),
            description: row.get("description"),
            tags,
            external_refs,
            repo_ids,
            task_list_ids,
            note_ids,
            created_at: Some(row.get("created_at")),
            updated_at: Some(row.get("updated_at")),
        })
    }

    async fn list(&self, query: Option<&ProjectQuery>) -> DbResult<ListResult<Project>> {
        let default_query = ProjectQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["title", "created_at", "updated_at"];

        let order_clause = build_order_clause(&query.page, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(&query.page);

        // Build conditions and bind values
        let mut conditions: Vec<String> = vec![];
        let mut bind_values: Vec<String> = vec![];

        // Tag filtering requires json_each join
        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());

        if let Some(tags) = &query.tags
            && !tags.is_empty()
        {
            let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
            conditions.push(format!("json_each.value IN ({})", placeholders.join(", ")));
            bind_values.extend(tags.clone());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Build SQL based on whether we need json_each
        let (sql, count_sql) = if needs_json_each {
            (
                format!(
                    "SELECT DISTINCT p.id, p.title, p.description, p.tags, p.external_refs, p.created_at, p.updated_at \
                     FROM project p, json_each(p.tags) {} {} {}",
                    where_clause, order_clause, limit_clause
                ),
                format!(
                    "SELECT COUNT(DISTINCT p.id) FROM project p, json_each(p.tags) {}",
                    where_clause
                ),
            )
        } else {
            (
                format!(
                    "SELECT id, title, description, tags, external_refs, created_at, updated_at FROM project {} {}",
                    order_clause, limit_clause
                ),
                "SELECT COUNT(*) FROM project".to_string(),
            )
        };

        // Execute main query
        let mut sql_query = sqlx::query(&sql);
        for value in &bind_values {
            sql_query = sql_query.bind(value);
        }

        let rows = sql_query
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let items: Vec<Project> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let external_refs_json: String = row.get("external_refs");
                let external_refs: Vec<String> =
                    serde_json::from_str(&external_refs_json).unwrap_or_default();
                Project {
                    id: row.get("id"),
                    title: row.get("title"),
                    description: row.get("description"),
                    tags,
                    external_refs,
                    repo_ids: vec![],
                    task_list_ids: vec![],
                    note_ids: vec![],
                    created_at: Some(row.get("created_at")),
                    updated_at: Some(row.get("updated_at")),
                }
            })
            .collect();

        // Execute count query
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

    async fn count(&self) -> DbResult<usize> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project")
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;
        Ok(count as usize)
    }

    async fn update(&self, project: &Project) -> DbResult<()> {
        // Validate project
        validate_project(project)?;

        let tags_json = serde_json::to_string(&project.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;
        let external_refs_json =
            serde_json::to_string(&project.external_refs).map_err(|e| DbError::Database {
                message: format!("Failed to serialize external_refs: {}", e),
            })?;

        // Use provided timestamp or generate if None
        let updated_at = project.updated_at.clone().unwrap_or_else(current_timestamp);

        let result = sqlx::query(
            "UPDATE project SET title = ?, description = ?, tags = ?, external_refs = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&project.title)
        .bind(&project.description)
        .bind(&tags_json)
        .bind(&external_refs_json)
        .bind(&updated_at)
        .bind(&project.id)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Project".to_string(),
                id: project.id.clone(),
            });
        }

        Ok(())
    }

    async fn delete(&self, id: &str) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM project WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Project".to_string(),
                id: id.to_string(),
            });
        }

        Ok(())
    }

    async fn search(
        &self,
        search_term: &str,
        query: Option<&ProjectQuery>,
    ) -> DbResult<ListResult<Project>> {
        let default_query = ProjectQuery::default();
        let query = query.unwrap_or(&default_query);

        // Sanitize FTS5 search query to prevent syntax errors
        // Same sanitization logic as Note search
        let fts_query = {
            // Strip FTS5-dangerous special characters
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

            // Handle unbalanced quotes
            let quote_count = cleaned.chars().filter(|c| *c == '"').count();
            let cleaned = if quote_count % 2 == 0 {
                cleaned
            } else {
                cleaned.replace('"', "")
            };

            // Handle empty/whitespace-only queries
            if cleaned.trim().is_empty() {
                return Ok(ListResult {
                    items: vec![],
                    total: 0,
                    limit: query.page.limit,
                    offset: query.page.offset.unwrap_or(0),
                });
            }

            // Detect advanced search features
            let has_boolean =
                cleaned.contains(" AND ") || cleaned.contains(" OR ") || cleaned.contains(" NOT ");
            let has_phrase = cleaned.contains('"');

            // Apply query transformation
            if has_boolean || has_phrase {
                cleaned
            } else {
                // Simple mode - add prefix matching
                cleaned
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .map(|term| format!("{}*", term))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        };

        let mut bind_values: Vec<String> = vec![fts_query];
        let mut where_conditions: Vec<String> = vec!["project_fts MATCH ?".to_string()];

        // Check if we need JOINs for tag filtering
        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());

        // Add tag filter if specified
        if needs_json_each {
            let tags = query.tags.as_ref().unwrap();
            let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
            where_conditions.push(format!("json_each.value IN ({})", placeholders.join(", ")));
            bind_values.extend(tags.clone());
        }

        let where_clause = format!("WHERE {}", where_conditions.join(" AND "));

        // Build ORDER BY clause
        let allowed_fields = ["title", "created_at", "updated_at"];
        let order_clause = {
            let sort_field = query
                .page
                .sort_by
                .as_deref()
                .filter(|f| allowed_fields.contains(f))
                .unwrap_or("created_at");

            let order = match query.page.sort_order.unwrap_or(SortOrder::Asc) {
                SortOrder::Asc => "ASC",
                SortOrder::Desc => "DESC",
            };

            format!("ORDER BY p.{} {}", sort_field, order)
        };

        // Build FROM clause with necessary JOINs
        let from_clause = if needs_json_each {
            "FROM project p INNER JOIN project_fts ON p.id = project_fts.id, json_each(p.tags)"
        } else {
            "FROM project p INNER JOIN project_fts ON p.id = project_fts.id"
        };

        // Count query
        let count_sql = format!(
            "SELECT COUNT(DISTINCT p.id) {} {}",
            from_clause, where_clause
        );

        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        for value in &bind_values {
            count_query = count_query.bind(value);
        }
        let total = count_query
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })? as usize;

        // Data query with LIMIT/OFFSET
        let limit = query.page.limit.unwrap_or(20);
        let offset = query.page.offset.unwrap_or(0);

        let data_sql = format!(
            "SELECT DISTINCT p.id, p.title, p.description, p.tags, p.external_refs, p.created_at, p.updated_at
             {}
             {}
             {}
             LIMIT ? OFFSET ?",
            from_clause, where_clause, order_clause
        );

        let mut data_query = sqlx::query(&data_sql);
        for value in &bind_values {
            data_query = data_query.bind(value);
        }
        data_query = data_query.bind(limit as i64);
        data_query = data_query.bind(offset as i64);

        let rows = data_query
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        // Convert rows to Project with empty relationship IDs
        // Search doesn't load relationships (use get() for that)
        let items: Vec<Project> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let external_refs_json: String = row.get("external_refs");

                Project {
                    id: row.get("id"),
                    title: row.get("title"),
                    description: row.get("description"),
                    tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                    external_refs: serde_json::from_str(&external_refs_json).unwrap_or_default(),
                    repo_ids: vec![],
                    task_list_ids: vec![],
                    note_ids: vec![],
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();

        Ok(ListResult {
            items,
            total,
            limit: query.page.limit,
            offset: query.page.offset.unwrap_or(0),
        })
    }
}
