//! Code analysis tool implementations
//!
//! Handles MCP tools for code analysis operations.
//! Follows SOLID principles - thin MCP layer delegating to service layer.

use crate::a6s;
use crate::analysis::get_analysis_path;
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

        // Spawn NEW a6s analysis pipeline in background
        let repo_id = params.0.repo_id.clone();
        let repo_path = PathBuf::from(&repo_path_str);

        tokio::spawn(async move {
            tracing::info!("Starting a6s analysis for repo: {}", repo_id);

            // Get commit hash (stub - could get from git later)
            let commit_hash = "HEAD";

            match a6s::analyze(&repo_path, &graph_path, commit_hash, None).await {
                Ok(stats) => {
                    tracing::info!(
                        "a6s analysis complete: {} symbols, {} edges resolved, {} dropped",
                        stats.symbols_registered,
                        stats.edges_resolved,
                        stats.edges_dropped
                    );
                }
                Err(e) => {
                    tracing::error!("a6s analysis failed: {:?}", e);
                }
            }
        });

        let response = json!({
            "status": "started",
            "message": format!("Analysis started (a6s pipeline) for repository {}. This will run in the background.", params.0.repo_id),
            "repo_id": params.0.repo_id,
            "pipeline": "a6s (scaffolding)",
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
