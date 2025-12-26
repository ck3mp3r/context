//! SQLite RepoRepository implementation.

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::{DbError, DbResult, ListQuery, ListResult, Repo, RepoRepository};

/// SQLx-backed repo repository.
pub struct SqliteRepoRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> RepoRepository for SqliteRepoRepository<'a> {
    async fn create(&self, repo: &Repo) -> DbResult<()> {
        sqlx::query("INSERT INTO repo (id, remote, path, created_at) VALUES (?, ?, ?, ?)")
            .bind(&repo.id)
            .bind(&repo.remote)
            .bind(&repo.path)
            .bind(&repo.created_at)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(())
    }

    async fn get(&self, id: &str) -> DbResult<Repo> {
        let row = sqlx::query("SELECT id, remote, path, created_at FROM repo WHERE id = ?")
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

        Ok(Repo {
            id: row.get("id"),
            remote: row.get("remote"),
            path: row.get("path"),
            created_at: row.get("created_at"),
        })
    }

    async fn list(&self, query: Option<&ListQuery>) -> DbResult<ListResult<Repo>> {
        let default_query = ListQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["remote", "path", "created_at"];

        // Build query with pagination and sorting
        let order_clause = build_order_clause(query, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(query);

        let sql = format!(
            "SELECT id, remote, path, created_at FROM repo {} {}",
            order_clause, limit_clause
        );

        // Get paginated results
        let rows = sqlx::query(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let items: Vec<Repo> = rows
            .into_iter()
            .map(|row| Repo {
                id: row.get("id"),
                remote: row.get("remote"),
                path: row.get("path"),
                created_at: row.get("created_at"),
            })
            .collect();

        // Get total count
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repo")
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

    async fn update(&self, repo: &Repo) -> DbResult<()> {
        let result = sqlx::query("UPDATE repo SET remote = ?, path = ? WHERE id = ?")
            .bind(&repo.remote)
            .bind(&repo.path)
            .bind(&repo.id)
            .execute(self.pool)
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
