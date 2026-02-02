//! SQLite SkillRepository implementation for skills backend.

use sqlx::{Row, SqlitePool};

use super::helpers::build_limit_offset_clause;
use crate::db::models::{SKILL_DESCRIPTION_MAX, Skill, SkillAttachment, SkillQuery};
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{DbError, DbResult, ListResult, SkillRepository};

/// SQLx-backed skill repository.
pub struct SqliteSkillRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

/// Standard column list for SELECT queries (without table alias)
const SKILL_COLS: &str = "id, name, description, content, tags, created_at, updated_at";

/// Standard column list for SELECT queries (with 's.' table alias)
const SKILL_COLS_ALIASED: &str =
    "s.id, s.name, s.description, s.content, s.tags, s.created_at, s.updated_at";

/// Parse a database row into a Skill struct (without project_ids)
fn row_to_skill(row: &sqlx::sqlite::SqliteRow) -> Skill {
    let tags_json: String = row.get("tags");
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

    Skill {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        content: row.get("content"),
        tags,
        project_ids: vec![], // Loaded separately via join table
        scripts: vec![],     // Loaded separately via skill_attachment table
        references: vec![],  // Loaded separately via skill_attachment table
        assets: vec![],      // Loaded separately via skill_attachment table
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

/// Validates skill for database storage.
///
/// Validation rules:
/// - name: required, not empty
/// - description: required, not empty, max 1024 chars  
/// - content: required, not empty, must start with '---' (YAML frontmatter delimiter)
fn validate_skill(skill: &Skill) -> DbResult<()> {
    let mut errors = Vec::new();

    // Validate name (required, not empty)
    if skill.name.trim().is_empty() {
        errors.push("Skill name cannot be empty".to_string());
    }

    // Validate description (required, not empty, max length)
    if skill.description.trim().is_empty() {
        errors.push("Skill description cannot be empty".to_string());
    } else if skill.description.len() > SKILL_DESCRIPTION_MAX {
        errors.push(format!(
            "Skill description exceeds maximum length of {} characters ({} chars)",
            SKILL_DESCRIPTION_MAX,
            skill.description.len()
        ));
    }

    // Validate content (required, not empty, must be YAML frontmatter + markdown)
    if skill.content.trim().is_empty() {
        errors.push("Skill content cannot be empty".to_string());
    } else if !skill.content.trim_start().starts_with("---") {
        errors.push("Skill content must start with '---' (YAML frontmatter delimiter)".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(DbError::Validation {
            message: errors.join("; "),
        })
    }
}

impl<'a> SkillRepository for SqliteSkillRepository<'a> {
    async fn create(&self, skill: &Skill) -> DbResult<Skill> {
        // Validate skill
        validate_skill(skill)?;

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
            r#"
            INSERT INTO skill (
                id, name, description, content, tags,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&skill.name)
        .bind(&skill.description)
        .bind(&skill.content)
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
            content: skill.content.clone(),
            tags: skill.tags.clone(),
            project_ids: skill.project_ids.clone(),
            scripts: skill.scripts.clone(),
            references: skill.references.clone(),
            assets: skill.assets.clone(),
            created_at: Some(created_at),
            updated_at: Some(updated_at),
        })
    }

    async fn get(&self, id: &str) -> DbResult<Skill> {
        let sql = format!("SELECT {} FROM skill WHERE id = ?", SKILL_COLS);
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if let Some(row) = row {
            let mut skill = row_to_skill(&row);

            // Load project relationships
            let project_ids: Vec<String> =
                sqlx::query_scalar("SELECT project_id FROM project_skill WHERE skill_id = ?")
                    .bind(id)
                    .fetch_all(self.pool)
                    .await
                    .map_err(|e| DbError::Database {
                        message: e.to_string(),
                    })?;

            skill.project_ids = project_ids;

            // Load attachment filenames grouped by type
            let (scripts, references, assets) = self.load_attachments(id).await?;
            skill.scripts = scripts;
            skill.references = references;
            skill.assets = assets;

            Ok(skill)
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
            (format!("DISTINCT {}", SKILL_COLS_ALIASED), from, "s.")
        } else {
            (SKILL_COLS.to_string(), "FROM skill".to_string(), "")
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
        let items: Vec<Skill> = rows.iter().map(row_to_skill).collect();
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
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM skill")
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;
        Ok(count as usize)
    }

    async fn update(&self, skill: &Skill) -> DbResult<()> {
        // Validate skill
        validate_skill(skill)?;

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
            SET name = ?, description = ?, content = ?, tags = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&skill.name)
        .bind(&skill.description)
        .bind(&skill.content)
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

        // Invalidate cache after successful update
        crate::skills::invalidate_cache(&skill.id)?;

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

        // Invalidate cache after successful delete
        crate::skills::invalidate_cache(id)?;

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
        where_conditions.push("(name LIKE ? OR description LIKE ? OR content LIKE ?)".to_string());
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
        let sql = format!("SELECT {} FROM skill {}", SKILL_COLS, where_clause);
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
        let items: Vec<Skill> = rows.iter().map(row_to_skill).collect();
        let total = items.len();
        Ok(ListResult {
            items,
            total,
            limit: query.page.limit,
            offset: query.page.offset.unwrap_or(0),
        })
    }

    async fn get_attachments(&self, skill_id: &str) -> DbResult<Vec<SkillAttachment>> {
        let rows = sqlx::query(
            "SELECT id, skill_id, type, filename, content, content_hash, mime_type, created_at, updated_at FROM skill_attachment WHERE skill_id = ? ORDER BY type, filename"
        )
        .bind(skill_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let attachments = rows
            .iter()
            .map(|row| SkillAttachment {
                id: row.get("id"),
                skill_id: row.get("skill_id"),
                type_: row.get("type"),
                filename: row.get("filename"),
                content: row.get("content"),
                content_hash: row.get("content_hash"),
                mime_type: row.get("mime_type"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(attachments)
    }

    async fn count_attachments(&self) -> DbResult<usize> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM skill_attachment")
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;
        Ok(count as usize)
    }

    async fn create_attachment(&self, attachment: &SkillAttachment) -> DbResult<SkillAttachment> {
        let id = if attachment.id.is_empty() {
            generate_entity_id()
        } else {
            attachment.id.clone()
        };

        let created_at = attachment
            .created_at
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(current_timestamp);
        let updated_at = attachment
            .updated_at
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(current_timestamp);

        sqlx::query(
            r#"
            INSERT INTO skill_attachment (
                id, skill_id, type, filename, content, content_hash, mime_type,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&attachment.skill_id)
        .bind(&attachment.type_)
        .bind(&attachment.filename)
        .bind(&attachment.content)
        .bind(&attachment.content_hash)
        .bind(&attachment.mime_type)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(SkillAttachment {
            id,
            skill_id: attachment.skill_id.clone(),
            type_: attachment.type_.clone(),
            filename: attachment.filename.clone(),
            content: attachment.content.clone(),
            content_hash: attachment.content_hash.clone(),
            mime_type: attachment.mime_type.clone(),
            created_at: Some(created_at),
            updated_at: Some(updated_at),
        })
    }

    async fn update_attachment(&self, attachment: &SkillAttachment) -> DbResult<()> {
        let updated_at = current_timestamp();

        sqlx::query(
            r#"
            UPDATE skill_attachment
            SET content = ?, content_hash = ?, mime_type = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&attachment.content)
        .bind(&attachment.content_hash)
        .bind(&attachment.mime_type)
        .bind(&updated_at)
        .bind(&attachment.id)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(())
    }

    async fn delete_attachment(&self, id: &str) -> DbResult<()> {
        sqlx::query("DELETE FROM skill_attachment WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(())
    }
}

// =============================================================================
// Internal helper methods for attachment management
// =============================================================================

impl<'a> SqliteSkillRepository<'a> {
    /// Load attachment filenames for a skill, grouped by type.
    /// Returns (scripts, references, assets) as three separate vectors.
    async fn load_attachments(
        &self,
        skill_id: &str,
    ) -> DbResult<(Vec<String>, Vec<String>, Vec<String>)> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT type, filename FROM skill_attachment WHERE skill_id = ? ORDER BY filename",
        )
        .bind(skill_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let mut scripts = Vec::new();
        let mut references = Vec::new();
        let mut assets = Vec::new();

        for (type_, filename) in rows {
            match type_.as_str() {
                "script" => scripts.push(filename),
                "reference" => references.push(filename),
                "asset" => assets.push(filename),
                _ => {
                    // Unknown type - skip with warning (could log here)
                }
            }
        }

        Ok((scripts, references, assets))
    }
}
