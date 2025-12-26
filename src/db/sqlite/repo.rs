//! SQLite RepoRepository implementation.

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::{DbError, DbResult, ListResult, Repo, RepoQuery, RepoRepository};

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

    async fn list(&self, query: Option<&RepoQuery>) -> DbResult<ListResult<Repo>> {
        let default_query = RepoQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["remote", "path", "created_at"];

        let order_clause = build_order_clause(&query.page, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(&query.page);

        let sql = format!(
            "SELECT id, remote, path, created_at FROM repo {} {}",
            order_clause, limit_clause
        );

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

        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repo")
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
