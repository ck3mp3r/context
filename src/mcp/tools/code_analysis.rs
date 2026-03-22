//! Code analysis tool implementations
//!
//! Handles MCP tools for code analysis operations.
//! Follows SOLID principles - thin MCP layer delegating to service layer.

use crate::analysis::service;
use crate::db::{Database, RepoRepository};
use crate::mcp::tools::map_db_error;
use crate::sync::get_data_dir;
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryCodeGraphParams {
    #[schemars(description = "Repository ID")]
    pub repo_id: String,
    #[schemars(description = "File path to query symbols for")]
    pub file_path: String,
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

        // Check if graph directory exists, if not initialize it
        let analysis_path = graph_path.join("analysis.nano");
        if !analysis_path.exists() {
            std::fs::create_dir_all(&analysis_path).map_err(|e| {
                McpError::internal_error("fs_error", Some(json!({ "error": e.to_string() })))
            })?;

            // Write schema
            let schema_path = analysis_path.join("schema.pg");
            std::fs::write(&schema_path, include_str!("../../analysis/schema.pg")).map_err(
                |e| McpError::internal_error("fs_error", Some(json!({ "error": e.to_string() }))),
            )?;

            // Initialize nanograph
            let init_output = std::process::Command::new("nanograph")
                .arg("init")
                .arg("--db")
                .arg(&analysis_path)
                .arg("--schema")
                .arg(&schema_path)
                .output()
                .map_err(|e| {
                    McpError::internal_error(
                        "nanograph_init_error",
                        Some(json!({ "error": e.to_string() })),
                    )
                })?;

            if !init_output.status.success() {
                let stderr = String::from_utf8_lossy(&init_output.stderr);
                return Err(McpError::internal_error(
                    "nanograph_init_failed",
                    Some(json!({ "error": stderr })),
                ));
            }
        }

        // Spawn analysis as background process
        // Analysis now detects languages automatically
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

    #[tool(description = "Query the code graph for symbols in a specific file")]
    pub async fn query_code_graph(
        &self,
        params: Parameters<QueryCodeGraphParams>,
    ) -> Result<CallToolResult, McpError> {
        let graph_path = get_analysis_path(&params.0.repo_id);
        let graph = crate::analysis::CodeGraph::new(&graph_path, &params.0.repo_id)
            .await
            .map_err(|e| {
                McpError::internal_error("graph_error", Some(json!({ "error": e.to_string() })))
            })?;

        let symbols = graph
            .query_symbols_in_file(&params.0.file_path)
            .await
            .map_err(|e| {
                McpError::internal_error("query_error", Some(json!({ "error": e.to_string() })))
            })?;

        let symbol_infos: Vec<_> = symbols
            .into_iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "kind": s.kind,
                    "file_path": s.file_path,
                    "start_line": s.start_line,
                    "end_line": s.end_line,
                    "signature": s.signature,
                })
            })
            .collect();

        let response = json!({
            "symbols": symbol_infos,
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

/// Get the analysis directory path for a repository
///
/// Uses the XDG-compliant data directory from sync::paths
fn get_analysis_path(repo_id: &str) -> PathBuf {
    get_data_dir().join("repos").join(repo_id)
}
