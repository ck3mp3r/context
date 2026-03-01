//! SQLite RepoRepository implementation.

use sqlx::{Row, SqlitePool};

use super::helpers::build_limit_offset_clause;
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{DbError, DbResult, ListResult, Repo, RepoQuery, RepoRepository};

/// SQLx-backed repo repository.
pub struct SqliteRepoRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

fn validate_repo(repo: &Repo) -> DbResult<()> {
    let mut errors = Vec::new();

    // Validate remote URL (required, not empty)
    if repo.remote.trim().is_empty() {
        errors.push("Repo remote URL cannot be empty".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(DbError::Validation {
            message: errors.join("; "),
        })
    }
}

impl<'a> RepoRepository for SqliteRepoRepository<'a> {
    async fn create(&self, repo: &Repo) -> DbResult<Repo> {
        // Validate repo
        validate_repo(repo)?;

        // Use provided ID if not empty, otherwise generate one
        let id = if repo.id.is_empty() {
            generate_entity_id()
        } else {
            repo.id.clone()
        };

        // Respect input timestamp or generate if None/empty (see utils.rs for policy)
        let created_at = repo
            .created_at
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| Some(current_timestamp()));

        let tags_json = serde_json::to_string(&repo.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;

        // Begin transaction for atomicity
        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&repo.remote)
            .bind(&repo.path)
            .bind(&tags_json)
            .bind(&created_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        // Insert project relationships
        for project_id in &repo.project_ids {
            sqlx::query("INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)")
                .bind(project_id)
                .bind(&id)
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
        }

        // Commit transaction
        tx.commit().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

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

        // Sanitize and prepare FTS5 query if search is requested
        let fts_query = if has_search {
            let search_term = query.search_query.as_ref().unwrap();
            super::helpers::sanitize_fts5_query(search_term)
        } else {
            None
        };

        // Decide on table alias usage
        let (select_cols, from_clause, order_field_prefix) =
            if needs_json_each || needs_project_join || fts_query.is_some() {
                // Need aliases when doing JOINs or FTS5
                let mut from = "FROM repo r".to_string();

                // Add FTS5 join if searching
                if fts_query.is_some() {
                    from.push_str("\nINNER JOIN repo_fts ON r.id = repo_fts.id");
                }

                if needs_project_join {
                    from.push_str("\nINNER JOIN project_repo pr ON r.id = pr.repo_id");
                    where_conditions.push("pr.project_id = ?".to_string());
                    bind_values.push(query.project_id.as_ref().unwrap().clone());
                }

                if needs_json_each {
                    from.push_str(", json_each(r.tags)");
                    let tags = query.tags.as_ref().unwrap();
                    let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
                    where_conditions
                        .push(format!("json_each.value IN ({})", placeholders.join(", ")));
                    bind_values.extend(tags.clone());
                }

                // Add FTS5 search condition
                if let Some(ref query_str) = fts_query {
                    where_conditions.push("repo_fts MATCH ?".to_string());
                    bind_values.push(query_str.clone());
                }

                (
                    "DISTINCT r.id, r.remote, r.path, r.tags, r.created_at",
                    from,
                    "r.",
                )
            } else {
                // No joins, simple query (no search)
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

        let count_sql = if needs_json_each || needs_project_join || fts_query.is_some() {
            format!(
                "SELECT COUNT(DISTINCT r.id) {} {}",
                from_clause, where_clause
            )
        } else if !where_clause.is_empty() {
            // Simple query but with WHERE clause (no joins)
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

    async fn count(&self) -> DbResult<usize> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repo")
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;
        Ok(count as usize)
    }

    async fn update(&self, repo: &Repo) -> DbResult<()> {
        // Validate repo
        validate_repo(repo)?;

        // Use transaction for atomicity
        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let tags_json = serde_json::to_string(&repo.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;

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
