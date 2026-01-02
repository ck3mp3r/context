//! SQLite RepoRepository implementation.

use sqlx::{Row, SqlitePool};

use super::helpers::build_limit_offset_clause;
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{DbError, DbResult, ListResult, Repo, RepoQuery, RepoRepository};

/// SQLx-backed repo repository.
pub struct SqliteRepoRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> RepoRepository for SqliteRepoRepository<'a> {
    async fn create(&self, repo: &Repo) -> DbResult<Repo> {
        // Use provided ID if not empty, otherwise generate one
        let id = if repo.id.is_empty() {
            generate_entity_id()
        } else {
            repo.id.clone()
        };

        // Always generate current timestamp - never use input timestamp
        let created_at = current_timestamp();

        let tags_json = serde_json::to_string(&repo.tags).unwrap_or_else(|_| "[]".to_string());

        sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&repo.remote)
            .bind(&repo.path)
            .bind(&tags_json)
            .bind(&created_at)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        // Insert project relationships
        for project_id in &repo.project_ids {
            sqlx::query("INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)")
                .bind(project_id)
                .bind(&id)
                .execute(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
        }

        Ok(Repo {
            id,
            remote: repo.remote.clone(),
            path: repo.path.clone(),
            tags: repo.tags.clone(),
            project_ids: repo.project_ids.clone(),
            created_at,
        })
    }

    async fn get(&self, id: &str) -> DbResult<Repo> {
        let row = sqlx::query("SELECT id, remote, path, tags, created_at FROM repo WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let row = row.ok_or(DbError::NotFound {
            entity_type: "Repo".to_string(),
            id: id.to_string(),
        })?;

        let tags_json: String = row.get("tags");
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

        // Get project relationships
        let project_ids: Vec<String> =
            sqlx::query_scalar("SELECT project_id FROM project_repo WHERE repo_id = ?")
                .bind(id)
                .fetch_all(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        Ok(Repo {
            id: row.get("id"),
            remote: row.get("remote"),
            path: row.get("path"),
            tags,
            project_ids,
            created_at: row.get("created_at"),
        })
    }

    async fn list(&self, query: Option<&RepoQuery>) -> DbResult<ListResult<Repo>> {
        let default_query = RepoQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["remote", "path", "created_at"];

        // Determine which JOINs are needed
        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());
        let needs_project_join = query.project_id.is_some();
        let has_search = query
            .search_query
            .as_ref()
            .is_some_and(|q| !q.trim().is_empty());

        let mut bind_values: Vec<String> = Vec::new();
        let mut where_conditions: Vec<String> = Vec::new();

        // Decide on table alias usage
        let (select_cols, from_clause, order_field_prefix) = if needs_json_each
            || needs_project_join
        {
            // Need aliases when doing JOINs
            let mut from = "FROM repo r".to_string();

            if needs_project_join {
                from.push_str("\nINNER JOIN project_repo pr ON r.id = pr.repo_id");
                where_conditions.push("pr.project_id = ?".to_string());
                bind_values.push(query.project_id.as_ref().unwrap().clone());
            }

            if needs_json_each {
                from.push_str(", json_each(r.tags)");
                let tags = query.tags.as_ref().unwrap();
                let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
                where_conditions.push(format!("json_each.value IN ({})", placeholders.join(", ")));
                bind_values.extend(tags.clone());
            }

            // Add search condition if present
            if has_search {
                let search_term = format!("%{}%", query.search_query.as_ref().unwrap());
                where_conditions.push(
                    "(LOWER(r.remote) LIKE LOWER(?) OR EXISTS (SELECT 1 FROM json_each(r.tags) WHERE LOWER(json_each.value) LIKE LOWER(?)))".to_string()
                );
                bind_values.push(search_term.clone());
                bind_values.push(search_term);
            }

            (
                "DISTINCT r.id, r.remote, r.path, r.tags, r.created_at",
                from,
                "r.",
            )
        } else {
            // No joins, simple query
            // Add search condition if present (no alias needed)
            if has_search {
                let search_term = format!("%{}%", query.search_query.as_ref().unwrap());
                where_conditions.push(
                    "(LOWER(remote) LIKE LOWER(?) OR EXISTS (SELECT 1 FROM json_each(tags) WHERE LOWER(json_each.value) LIKE LOWER(?)))".to_string()
                );
                bind_values.push(search_term.clone());
                bind_values.push(search_term);
            }

            (
                "id, remote, path, tags, created_at",
                "FROM repo".to_string(),
                "",
            )
        };

        // Build WHERE clause
        let where_clause = if !where_conditions.is_empty() {
            format!("WHERE {}", where_conditions.join(" AND "))
        } else {
            String::new()
        };

        // Build ORDER BY with proper prefixes
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
        let order_clause = format!(
            "ORDER BY {}{} {}",
            order_field_prefix, sort_field, sort_order
        );

        let limit_clause = build_limit_offset_clause(&query.page);

        // Build final SQL
        let sql = format!(
            "SELECT {} {} {} {} {}",
            select_cols, from_clause, where_clause, order_clause, limit_clause
        );

        let count_sql = if needs_json_each || needs_project_join {
            format!(
                "SELECT COUNT(DISTINCT r.id) {} {}",
                from_clause, where_clause
            )
        } else if !where_clause.is_empty() {
            // Simple query but with WHERE clause (e.g., search only)
            format!("SELECT COUNT(*) FROM repo {}", where_clause)
        } else {
            "SELECT COUNT(*) FROM repo".to_string()
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

        let items: Vec<Repo> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Repo {
                    id: row.get("id"),
                    remote: row.get("remote"),
                    path: row.get("path"),
                    tags,
                    project_ids: vec![], // Empty by default - relationships managed separately
                    created_at: row.get("created_at"),
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

    async fn update(&self, repo: &Repo) -> DbResult<()> {
        // Use transaction for atomicity
        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let tags_json = serde_json::to_string(&repo.tags).unwrap_or_else(|_| "[]".to_string());

        let result = sqlx::query("UPDATE repo SET remote = ?, path = ?, tags = ? WHERE id = ?")
            .bind(&repo.remote)
            .bind(&repo.path)
            .bind(&tags_json)
            .bind(&repo.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Repo".to_string(),
                id: repo.id.clone(),
            });
        }

        // Update project relationships (replace all)
        // Delete existing relationships
        sqlx::query("DELETE FROM project_repo WHERE repo_id = ?")
            .bind(&repo.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        // Insert new relationships
        for project_id in &repo.project_ids {
            sqlx::query("INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)")
                .bind(project_id)
                .bind(&repo.id)
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
        let result = sqlx::query("DELETE FROM repo WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Repo".to_string(),
                id: id.to_string(),
            });
        }

        Ok(())
    }
}
