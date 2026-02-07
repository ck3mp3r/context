//! MCP tools for Skill management.

use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    schemars::JsonSchema,
    tool, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

use crate::api::notifier::ChangeNotifier;
use crate::db::SkillRepository;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListSkillsParams {
    #[schemars(
        description = "FTS5 search query (optional). If provided, performs full-text search. Examples: 'rust AND async' (Boolean), 'phrase', 'term*' (prefix), 'NOT deprecated' (exclude), 'api AND (error OR bug)' (complex)"
    )]
    pub query: Option<String>,
    #[schemars(description = "Filter by tags")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Filter by project ID")]
    pub project_id: Option<String>,
    #[schemars(description = "Maximum number of items to return (default: 10, max: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of items to skip")]
    pub offset: Option<usize>,
    #[schemars(description = "Field to sort by (name, created_at). Default: created_at")]
    pub sort: Option<String>,
    #[schemars(description = "Sort order (asc, desc). Default: asc")]
    pub order: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetSkillParams {
    #[schemars(description = "Skill ID")]
    pub skill_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateSkillParams {
    #[schemars(description = "Skill ID to update")]
    pub skill_id: String,
    #[schemars(description = "New tags (optional). Replaces all existing tags when provided.")]
    pub tags: Option<Vec<String>>,
    #[schemars(
        description = "New project IDs (optional). Replaces all existing project links when provided."
    )]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct SkillTools<D: crate::db::Database> {
    db: Arc<D>,
    #[allow(dead_code)] // Will be used for change notifications
    notifier: ChangeNotifier,
    skills_dir: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: crate::db::Database + 'static> SkillTools<D> {
    pub fn new(db: Arc<D>, notifier: ChangeNotifier, skills_dir: PathBuf) -> Self {
        Self {
            db,
            notifier,
            skills_dir,
            tool_router: Self::tool_router(),
        }
    }

    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    // -- Tools to implement (following notes pattern)

    #[tool(
        description = "List skills with optional full-text search. Provide 'query' parameter to search, omit to list all. Supports filtering, sorting, and pagination."
    )]
    pub async fn list_skills(
        &self,
        params: Parameters<ListSkillsParams>,
    ) -> Result<CallToolResult, McpError> {
        let db_query = crate::db::SkillQuery {
            page: crate::db::PageSort {
                limit: params.0.limit,
                offset: params.0.offset,
                sort_by: params.0.sort.clone(),
                sort_order: match params.0.order.as_deref() {
                    Some("desc") => Some(crate::db::SortOrder::Desc),
                    Some("asc") => Some(crate::db::SortOrder::Asc),
                    _ => None,
                },
            },
            tags: params.0.tags.clone(),
            project_id: params.0.project_id.clone(),
        };

        // Route to search if query is provided, otherwise list all
        let result = if let Some(query) = &params.0.query {
            self.db
                .skills()
                .search(query, Some(&db_query))
                .await
                .map_err(|e| {
                    McpError::internal_error(
                        "database_error",
                        Some(serde_json::json!({"error": e.to_string()})),
                    )
                })?
        } else {
            self.db.skills().list(Some(&db_query)).await.map_err(|e| {
                McpError::internal_error(
                    "database_error",
                    Some(serde_json::json!({"error": e.to_string()})),
                )
            })?
        };

        let response = json!({
            "items": result.items,
            "total": result.total,
            "limit": result.limit,
            "offset": result.offset,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap(),
        )]))
    }

    #[tool(description = "Get a skill by ID")]
    pub async fn get_skill(
        &self,
        params: Parameters<GetSkillParams>,
    ) -> Result<CallToolResult, McpError> {
        let skill = self
            .db
            .skills()
            .get(&params.0.skill_id)
            .await
            .map_err(|e| match e {
                crate::db::DbError::NotFound { .. } => McpError::resource_not_found(
                    "skill_not_found",
                    Some(serde_json::json!({"error": e.to_string()})),
                ),
                _ => McpError::internal_error(
                    "database_error",
                    Some(serde_json::json!({"error": e.to_string()})),
                ),
            })?;

        // Extract attachments to cache if any exist
        let cache_path = if !skill.scripts.is_empty()
            || !skill.references.is_empty()
            || !skill.assets.is_empty()
        {
            let attachments = self
                .db
                .skills()
                .get_attachments(&params.0.skill_id)
                .await
                .map_err(|e| {
                    McpError::internal_error(
                        "database_error",
                        Some(serde_json::json!({"error": e.to_string()})),
                    )
                })?;

            // Parse skill name from content for cache directory
            let skill_name =
                crate::skills::parse_skill_name_from_content(&skill.content).map_err(|e| {
                    McpError::internal_error(
                        "parse_error",
                        Some(serde_json::json!({"error": e.to_string()})),
                    )
                })?;

            let cache_dir = crate::skills::extract_attachments(
                &self.skills_dir,
                &skill_name,
                &skill.content,
                &attachments,
            )
            .map_err(|e| {
                McpError::internal_error(
                    "cache_error",
                    Some(serde_json::json!({"error": e.to_string()})),
                )
            })?;

            Some(cache_dir.to_string_lossy().to_string())
        } else {
            None
        };

        let mut response = serde_json::to_value(&skill).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Add cache_path to response
        if let Some(obj) = response.as_object_mut() {
            obj.insert("cache_path".to_string(), json!(cache_path));
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap(),
        )]))
    }

    #[tool(
        description = "Update skill tags and/or project_ids. Allows partial updates - update tags without changing projects, or vice versa."
    )]
    pub async fn update_skill(
        &self,
        params: Parameters<UpdateSkillParams>,
    ) -> Result<CallToolResult, McpError> {
        // Fetch existing skill
        let mut skill = self
            .db
            .skills()
            .get(&params.0.skill_id)
            .await
            .map_err(|e| match e {
                crate::db::DbError::NotFound { .. } => McpError::resource_not_found(
                    "skill_not_found",
                    Some(serde_json::json!({"error": e.to_string()})),
                ),
                _ => McpError::internal_error(
                    "database_error",
                    Some(serde_json::json!({"error": e.to_string()})),
                ),
            })?;

        // Update tags if provided
        if let Some(tags) = params.0.tags {
            skill.tags = tags;
        }

        // Update project_ids if provided
        if let Some(project_ids) = params.0.project_ids {
            skill.project_ids = project_ids;
        }

        // Save updated skill
        self.db.skills().update(&skill).await.map_err(|e| {
            McpError::internal_error(
                "database_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Return updated skill
        let updated_skill = self
            .db
            .skills()
            .get(&params.0.skill_id)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "database_error",
                    Some(serde_json::json!({"error": e.to_string()})),
                )
            })?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&updated_skill).unwrap(),
        )]))
    }
}
