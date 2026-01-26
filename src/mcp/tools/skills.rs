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

// Parameter structs will live here (TDD: add tests expecting these)

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
    #[schemars(description = "Linked projects (optional)")]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteSkillParams {
    #[schemars(description = "Skill ID")]
    pub skill_id: String,
}

#[derive(Clone)]
pub struct SkillTools<D: crate::db::Database> {
    db: Arc<D>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: crate::db::Database + 'static> SkillTools<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    // minimal tool to satisfy wiring in future
    #[tool(description = "List skills placeholder")]
    pub async fn list_skills(
        &self,
        _params: Parameters<ListSkillsParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&json!({"items":[], "total":0, "limit":0, "offset":0})).unwrap(),
        )]))
    }
}
