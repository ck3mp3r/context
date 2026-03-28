//! Code analysis tool implementations
//!
//! Handles MCP tools for code analysis operations.
//! Follows SOLID principles - thin MCP layer delegating to service layer.

use crate::analysis::{get_analysis_path, service};
use crate::db::{Database, RepoRepository};
use crate::mcp::tools::map_db_error;
use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content},
    schemars,
    schemars::JsonSchema,
    tool, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

// Parameter types
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeCodeParams {
    #[schemars(description = "Repository ID from c5t database")]
    pub repo_id: String,
}

/// Code analysis tools
///
/// # SOLID Principles
/// - **Single Responsibility**: MCP interface only
/// - **Dependency Inversion**: Depends on Database trait and service layer
#[derive(Clone)]
pub struct CodeAnalysisTools<D: Database> {
    db: Arc<D>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> CodeAnalysisTools<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    #[tool(description = "Start analyzing a repository's code in the background")]
    pub async fn analyze_code(
        &self,
        params: Parameters<AnalyzeCodeParams>,
    ) -> Result<CallToolResult, McpError> {
        // Load repository
        let repo = self
            .db
            .repos()
            .get(&params.0.repo_id)
            .await
            .map_err(map_db_error)?;

        let repo_path_str = repo.path.ok_or_else(|| {
            McpError::invalid_params(
                "missing_path",
                Some(json!({ "message": "Repository has no local path configured" })),
            )
        })?;

        let graph_path = get_analysis_path(&params.0.repo_id);

        // Spawn analysis as background process
        // CodeGraph::new() handles init if needed
        let repo_id = params.0.repo_id.clone();
        tokio::spawn(async move {
            let _ =
                service::analyze_repository(&PathBuf::from(&repo_path_str), &repo_id, &graph_path)
                    .await;
        });

        let response = json!({
            "status": "started",
            "message": format!("Analysis started for repository {}. This will run in the background.", params.0.repo_id),
            "repo_id": params.0.repo_id,
        });

        let content = serde_json::to_string_pretty(&response).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(json!({ "error": e.to_string() })),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }
}
