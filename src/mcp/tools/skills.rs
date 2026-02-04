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
use std::sync::Arc;

use crate::api::notifier::ChangeNotifier;
use crate::db::SkillRepository;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListSkillsParams {
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

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchSkillsParams {
    #[schemars(
        description = "FTS5 search query. Examples: 'rust AND async' (Boolean), 'phrase', 'term*' (prefix), 'NOT deprecated' (exclude), 'api AND (error OR bug)' (complex)"
    )]
    pub query: String,
    #[schemars(
        description = "Filter results by tags (optional). Can combine with search to find e.g. session skills matching a term."
    )]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Filter by project ID (optional)")]
    pub project_id: Option<String>,
    #[schemars(description = "Maximum number of results to return (default: 10, max: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of results to skip (optional)")]
    pub offset: Option<usize>,
    #[schemars(description = "Field to sort by (name, created_at). Default: created_at")]
    pub sort: Option<String>,
    #[schemars(description = "Sort order (asc, desc). Default: asc")]
    pub order: Option<String>,
}

#[derive(Clone)]
pub struct SkillTools<D: crate::db::Database> {
    db: Arc<D>,
    #[allow(dead_code)] // Will be used for change notifications
    notifier: ChangeNotifier,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: crate::db::Database + 'static> SkillTools<D> {
    pub fn new(db: Arc<D>, notifier: ChangeNotifier) -> Self {
        Self {
            db,
            notifier,
            tool_router: Self::tool_router(),
        }
    }

    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    // -- Tools to implement (following notes pattern)

    // minimal tool to satisfy wiring in future
    #[tool(description = "List skills placeholder")]
    pub async fn list_skills(
        &self,
        params: Parameters<ListSkillsParams>,
    ) -> Result<CallToolResult, McpError> {
        let query = crate::db::SkillQuery {
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

        let result = self.db.skills().list(Some(&query)).await.map_err(|e| {
            McpError::internal_error(
                "database_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

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

            let cache_dir =
                crate::skills::extract_attachments(&skill_name, &skill.content, &attachments)
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
        description = "Full-text search skills (FTS5) with optional sorting. Supports: Boolean AND/OR/NOT, phrase, prefix. Filter results by tags/project_id. Sort by name, created_at. Returns metadata only (no large fields)."
    )]
    pub async fn search_skills(
        &self,
        params: Parameters<SearchSkillsParams>,
    ) -> Result<CallToolResult, McpError> {
        // Build query
        let query = crate::db::SkillQuery {
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

        let result = self
            .db
            .skills()
            .search(&params.0.query, Some(&query))
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "database_error",
                    Some(serde_json::json!({"error": e.to_string()})),
                )
            })?;

        let response = serde_json::json!({
            "items": result.items,
            "total": result.total,
            "limit": result.limit,
            "offset": result.offset,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap(),
        )]))
    }
}
