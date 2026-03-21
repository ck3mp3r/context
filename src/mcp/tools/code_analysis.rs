//! Code analysis tool implementations
//!
//! Handles MCP tools for code analysis operations.
//! Follows SOLID principles - thin MCP layer delegating to service layer.

use crate::analysis::{languages::rust::RustExtractor, service};
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

    #[tool(description = "Analyze a repository's code and extract symbols into the code graph")]
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
        let repo_path = PathBuf::from(&repo_path_str);

        let graph_path = get_analysis_path(&params.0.repo_id);

        // Use service layer with dependency injection (SOLID!)
        let extractor = RustExtractor;
        let result =
            service::analyze_repository(&repo_path, &params.0.repo_id, &graph_path, &extractor)
                .await
                .map_err(|e| {
                    McpError::internal_error(
                        "analysis_error",
                        Some(json!({ "error": e.to_string() })),
                    )
                })?;

        let response = json!({
            "files_analyzed": result.files_analyzed,
            "symbols_extracted": result.symbols_extracted,
            "relationships_created": result.relationships_created,
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

fn get_analysis_path(repo_id: &str) -> PathBuf {
    // Inline version of get_data_dir to avoid module privacy issues
    let data_dir = std::env::var("C5T_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            #[cfg(target_os = "macos")]
            {
                let home = std::env::var("HOME").expect("HOME not set");
                PathBuf::from(home).join("Library/Application Support/c5t")
            }
            #[cfg(not(target_os = "macos"))]
            {
                let home = std::env::var("HOME").expect("HOME not set");
                PathBuf::from(home).join(".local/share/c5t")
            }
        });

    data_dir.join("repos").join(repo_id)
}
