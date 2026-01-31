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

use crate::api::notifier::{ChangeNotifier, UpdateMessage};
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateSkillParams {
    #[schemars(description = "Skill name")]
    pub name: String,
    #[schemars(description = "Description")]
    pub description: Option<String>,
    #[schemars(description = "Instructions")]
    pub instructions: Option<String>,
    #[schemars(description = "Tags")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "License (e.g., 'MIT', 'Apache-2.0')")]
    pub license: Option<String>,
    #[schemars(description = "Compatibility requirements (e.g., 'opencode>=0.1.0')")]
    pub compatibility: Option<String>,
    #[schemars(description = "Allowed tools")]
    pub allowed_tools: Option<Vec<String>>,
    #[schemars(description = "Arbitrary metadata as JSON")]
    pub metadata: Option<serde_json::Value>,
    #[schemars(description = "Origin URL (e.g., 'https://github.com/user/repo')")]
    pub origin_url: Option<String>,
    #[schemars(description = "Origin ref (e.g., 'main', 'v1.0.0')")]
    pub origin_ref: Option<String>,
    #[schemars(description = "Linked projects")]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateSkillParams {
    #[schemars(description = "Skill ID")]
    pub skill_id: String,
    #[schemars(description = "Name (optional)")]
    pub name: Option<String>,
    #[schemars(description = "Description (optional)")]
    pub description: Option<String>,
    #[schemars(description = "Instructions (optional)")]
    pub instructions: Option<String>,
    #[schemars(description = "Tags (optional)")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "License (optional)")]
    pub license: Option<String>,
    #[schemars(description = "Compatibility requirements (optional)")]
    pub compatibility: Option<String>,
    #[schemars(description = "Allowed tools (optional)")]
    pub allowed_tools: Option<Vec<String>>,
    #[schemars(description = "Metadata (optional)")]
    pub metadata: Option<serde_json::Value>,
    #[schemars(description = "Origin URL (optional)")]
    pub origin_url: Option<String>,
    #[schemars(description = "Origin ref (optional)")]
    pub origin_ref: Option<String>,
    #[schemars(description = "Linked projects (optional)")]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteSkillParams {
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

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&skill).unwrap(),
        )]))
    }

    #[tool(description = "Create a new skill")]
    pub async fn create_skill(
        &self,
        params: Parameters<CreateSkillParams>,
    ) -> Result<CallToolResult, McpError> {
        let skill = crate::db::Skill {
            id: crate::db::utils::generate_entity_id(),
            name: params.0.name.clone(),
            description: params.0.description.clone(),
            instructions: params.0.instructions.clone(),
            tags: params.0.tags.clone().unwrap_or_default(),
            license: params.0.license.clone(),
            compatibility: params.0.compatibility.clone(),
            allowed_tools: params
                .0
                .allowed_tools
                .clone()
                .map(|v| serde_json::to_string(&v).unwrap()),
            metadata: params.0.metadata.clone(),
            origin_url: params.0.origin_url.clone(),
            origin_ref: params.0.origin_ref.clone(),
            origin_fetched_at: None,
            origin_metadata: None,
            project_ids: params.0.project_ids.clone().unwrap_or_default(),
            created_at: None,
            updated_at: None,
        };

        let created = self.db.skills().create(&skill).await.map_err(|e| {
            McpError::internal_error(
                "database_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Broadcast SkillCreated notification
        self.notifier.notify(UpdateMessage::SkillCreated {
            skill_id: created.id.clone(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&created).unwrap(),
        )]))
    }

    #[tool(description = "Update a skill")]
    pub async fn update_skill(
        &self,
        params: Parameters<UpdateSkillParams>,
    ) -> Result<CallToolResult, McpError> {
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

        if let Some(name) = &params.0.name {
            skill.name = name.clone();
        }
        if let Some(description) = &params.0.description {
            skill.description = Some(description.clone());
        }
        if let Some(instructions) = &params.0.instructions {
            skill.instructions = Some(instructions.clone());
        }
        if let Some(tags) = &params.0.tags {
            skill.tags = tags.clone();
        }
        if let Some(license) = &params.0.license {
            skill.license = Some(license.clone());
        }
        if let Some(compatibility) = &params.0.compatibility {
            skill.compatibility = Some(compatibility.clone());
        }
        if let Some(allowed_tools) = &params.0.allowed_tools {
            skill.allowed_tools = Some(serde_json::to_string(allowed_tools).unwrap());
        }
        if let Some(metadata) = &params.0.metadata {
            skill.metadata = Some(metadata.clone());
        }
        if let Some(origin_url) = &params.0.origin_url {
            skill.origin_url = Some(origin_url.clone());
        }
        if let Some(origin_ref) = &params.0.origin_ref {
            skill.origin_ref = Some(origin_ref.clone());
        }
        if let Some(project_ids) = &params.0.project_ids {
            skill.project_ids = project_ids.clone();
        }

        self.db.skills().update(&skill).await.map_err(|e| {
            McpError::internal_error(
                "database_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        // Broadcast SkillUpdated notification
        self.notifier.notify(UpdateMessage::SkillUpdated {
            skill_id: skill.id.clone(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&skill).unwrap(),
        )]))
    }

    #[tool(description = "Delete a skill")]
    pub async fn delete_skill(
        &self,
        params: Parameters<DeleteSkillParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .skills()
            .delete(&params.0.skill_id)
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

        // Broadcast SkillDeleted notification
        self.notifier.notify(UpdateMessage::SkillDeleted {
            skill_id: params.0.skill_id.clone(),
        });

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Skill {} deleted successfully",
            params.0.skill_id
        ))]))
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
