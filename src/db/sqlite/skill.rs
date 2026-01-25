//! SQLite SkillRepository implementation for skills backend.

use sqlx::{Row, SqlitePool};

use super::helpers::build_limit_offset_clause;
use crate::db::models::{Skill, SkillQuery};
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{DbError, DbResult, ListResult, SkillRepository};

/// SQLx-backed skill repository.
pub struct SqliteSkillRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> SkillRepository for SqliteSkillRepository<'a> {
    async fn create(&self, skill: &Skill) -> DbResult<Skill> {
        // Use provided ID if not empty, otherwise generate one
        let id = if skill.id.is_empty() {
            generate_entity_id()
        } else {
            skill.id.clone()
        };

        let created_at = skill
            .created_at
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(current_timestamp);
        let updated_at = skill
            .updated_at
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(current_timestamp);

        let tags_json = serde_json::to_string(&skill.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;

        sqlx::query(
            r#"INSERT INTO skill (id, name, description, instructions, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&id)
        .bind(&skill.name)
        .bind(&skill.description)
        .bind(&skill.instructions)
        .bind(tags_json)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Insert project relationships
        for project_id in &skill.project_ids {
            sqlx::query("INSERT INTO project_skill (project_id, skill_id) VALUES (?, ?)")
                .bind(project_id)
                .bind(&id)
                .execute(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
        }

        Ok(Skill {
            id,
            name: skill.name.clone(),
            description: skill.description.clone(),
            instructions: skill.instructions.clone(),
            tags: skill.tags.clone(),
            project_ids: skill.project_ids.clone(),
            created_at: Some(created_at),
            updated_at: Some(updated_at),
        })
    }

    async fn get(&self, id: &str) -> DbResult<Skill> {
        let row = sqlx::query(
            "SELECT id, name, description, instructions, tags, created_at, updated_at FROM skill WHERE id = ?",
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
            let project_ids: Vec<String> =
                sqlx::query_scalar("SELECT project_id FROM project_skill WHERE skill_id = ?")
                    .bind(id)
                    .fetch_all(self.pool)
                    .await
                    .map_err(|e| DbError::Database {
                        message: e.to_string(),
                    })?;
            Ok(Skill {
                id: row.get("id"),
                name: row.get("name"),
                description: row.get("description"),
                instructions: row.get("instructions"),
                tags,
                project_ids,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
        } else {
            Err(DbError::NotFound {
                entity_type: "Skill".to_string(),
                id: id.to_string(),
            })
        }
    }

    async fn list(&self, query: Option<&SkillQuery>) -> DbResult<ListResult<Skill>> {
        let default_query = SkillQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["name", "created_at", "updated_at"];

        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());
        let needs_project_join = query.project_id.is_some();
        let mut bind_values: Vec<String> = Vec::new();
        let mut where_conditions: Vec<String> = Vec::new();
        let (select_cols, from_clause, order_field_prefix) = if needs_json_each
            || needs_project_join
        {
            let mut from = "FROM skill s".to_string();
            if needs_project_join {
                from.push_str("\nINNER JOIN project_skill ps ON s.id = ps.skill_id");
                where_conditions.push("ps.project_id = ?".to_string());
                bind_values.push(query.project_id.as_ref().unwrap().clone());
            }
            if needs_json_each {
                from.push_str(", json_each(s.tags)");
                let tags = query.tags.as_ref().unwrap();
                let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
                where_conditions.push(format!("json_each.value IN ({})", placeholders.join(", ")));
                bind_values.extend(tags.clone());
            }
            (
                "DISTINCT s.id, s.name, s.description, s.instructions, s.tags, s.created_at, s.updated_at",
                from,
                "s.",
            )
        } else {
            (
                "id, name, description, instructions, tags, created_at, updated_at",
                "FROM skill".to_string(),
                "",
            )
        };
        let where_clause = if !where_conditions.is_empty() {
            format!("WHERE {}", where_conditions.join(" AND "))
        } else {
            String::new()
        };
        let order_clause = {
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
            format!(
                "ORDER BY {}{} {}",
                order_field_prefix, sort_field, sort_order
            )
        };
        let limit_clause = build_limit_offset_clause(&query.page);
        let sql = format!(
            "SELECT {} {} {} {} {}",
            select_cols, from_clause, where_clause, order_clause, limit_clause
        );
        let count_sql = if needs_json_each || needs_project_join {
            format!(
                "SELECT COUNT(DISTINCT s.id) {} {}",
                from_clause, where_clause
            )
        } else {
            format!("SELECT COUNT(*) FROM skill {}", where_clause)
        };
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
        let items: Vec<Skill> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Skill {
                    id: row.get("id"),
                    name: row.get("name"),
                    description: row.get("description"),
                    instructions: row.get("instructions"),
                    tags,
                    project_ids: vec![], // relationships managed separately
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();
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

    async fn update(&self, skill: &Skill) -> DbResult<()> {
        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;
        let tags_json = serde_json::to_string(&skill.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;
        let updated_at = skill
            .updated_at
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(current_timestamp);
        let result = sqlx::query(
            r#"
            UPDATE skill
            SET name = ?, description = ?, instructions = ?, tags = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&skill.name)
        .bind(&skill.description)
        .bind(&skill.instructions)
        .bind(tags_json)
        .bind(&updated_at)
        .bind(&skill.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Skill".to_string(),
                id: skill.id.clone(),
            });
        }
        // Sync project relationships (delete old, insert new)
        sqlx::query("DELETE FROM project_skill WHERE skill_id = ?")
            .bind(&skill.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;
        for project_id in &skill.project_ids {
            sqlx::query("INSERT INTO project_skill (project_id, skill_id) VALUES (?, ?)")
                .bind(project_id)
                .bind(&skill.id)
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
        let result = sqlx::query("DELETE FROM skill WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Skill".to_string(),
                id: id.to_string(),
            });
        }
        Ok(())
    }

    async fn search(
        &self,
        search_term: &str,
        query: Option<&SkillQuery>,
    ) -> DbResult<ListResult<Skill>> {
        // Simple LIKE search for now (upgradeable to FTS later)
        let default_query = SkillQuery::default();
        let query = query.unwrap_or(&default_query);
        let mut bind_values: Vec<String> = Vec::new();
        let mut where_conditions: Vec<String> = Vec::new();
        where_conditions
            .push("(name LIKE ? OR description LIKE ? OR instructions LIKE ?)".to_string());
        let like_term = format!("%{}%", search_term);
        bind_values.push(like_term.clone());
        bind_values.push(like_term.clone());
        bind_values.push(like_term);
        if let Some(tags) = &query.tags
            && !tags.is_empty()
        {
            let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
            where_conditions.push(format!(
        "id IN (SELECT skill.id FROM skill, json_each(skill.tags) WHERE json_each.value IN ({}))",
        placeholders.join(", ")
    ));
            bind_values.extend(tags.clone());
        }
        if let Some(proj) = &query.project_id {
            where_conditions.push(
                "id IN (SELECT skill_id FROM project_skill WHERE project_id = ?)".to_string(),
            );
            bind_values.push(proj.clone());
        }
        let where_clause = format!("WHERE {}", where_conditions.join(" AND "));
        let sql = format!(
            "SELECT id, name, description, instructions, tags, created_at, updated_at FROM skill {}",
            where_clause
        );
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
        let items: Vec<Skill> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Skill {
                    id: row.get("id"),
                    name: row.get("name"),
                    description: row.get("description"),
                    instructions: row.get("instructions"),
                    tags,
                    project_ids: vec![], // relationships managed separately
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();
        let total = items.len();
        Ok(ListResult {
            items,
            total,
            limit: query.page.limit,
            offset: query.page.offset.unwrap_or(0),
        })
    }
}
