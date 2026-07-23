//! Code query tool implementations
//!
//! Provides MCP tools for querying the code graph using SurrealDB.
//! Follows SOLID principles - thin MCP layer delegating to CodeGraph.

use crate::a6s::store::CodeGraph;
use crate::a6s::store::surrealdb;
use crate::a6s::tracker::{AnalysisStatus, AnalysisTracker};
use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock},
    schemars,
    schemars::JsonSchema,
    tool, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

// ============================================================================
// Parameter types
// ============================================================================

/// Parameters for the `describe_schema` MCP tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DescribeSchemaParams {
    #[schemars(description = "Repository ID from c5t database")]
    pub repo_id: String,
}

/// Parameters for the `list_queries` MCP tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListQueriesParams {
    #[schemars(description = "Repository ID from c5t database")]
    pub repo_id: String,
}

/// Parameters for the `query_code_graph` MCP tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct QueryCodeGraphParams {
    #[schemars(description = "Repository ID")]
    #[serde(default)]
    pub repo_id: String,

    #[schemars(description = "Query name to execute or save (optional)")]
    pub query_name: Option<String>,

    #[schemars(
        description = "Query definition with @description/@instruction annotations (optional)"
    )]
    pub query_definition: Option<String>,

    #[schemars(description = "Query parameters as JSON object (optional)")]
    pub variables: Option<HashMap<String, serde_json::Value>>,
}

// ============================================================================
// Code query tools
// ============================================================================

/// Code query tools
///
/// # SOLID Principles
/// - **Single Responsibility**: MCP interface only
/// - **Dependency Inversion**: Depends on CodeGraph abstraction
pub struct CodeQueryTools {
    analysis_db: Arc<surrealdb::SurrealDbConnection>,
    tracker: AnalysisTracker,
    tool_router: ToolRouter<Self>,
}

impl CodeQueryTools {
    pub fn new(analysis_db: Arc<surrealdb::SurrealDbConnection>, tracker: AnalysisTracker) -> Self {
        Self {
            analysis_db,
            tracker,
            tool_router: Self::tool_router(),
        }
    }

    /// Build a tracker-aware error when analysis data is not available.
    pub fn analysis_not_ready_error_static(
        tracker: &AnalysisTracker,
        repo_id: &str,
        original_error: &str,
    ) -> McpError {
        let status_msg = match tracker.get(repo_id) {
            Some(AnalysisStatus::Analyzing { .. }) => {
                format!(
                    "Analysis is currently in progress for repository {}. Try again shortly.",
                    repo_id
                )
            }
            Some(AnalysisStatus::Failed { error }) => {
                format!(
                    "Analysis failed for repository {}: {}. Re-run code_analyze to retry. (Original error: {})",
                    repo_id, error, original_error
                )
            }
            _ => {
                format!(
                    "No analysis found for repository {}. Run code_analyze first. (Original error: {})",
                    repo_id, original_error
                )
            }
        };
        McpError::invalid_params(
            "analysis_not_ready",
            Some(json!({ "message": status_msg, "repo_id": repo_id })),
        )
    }

    fn analysis_not_ready_error(&self, repo_id: &str, original_error: &str) -> McpError {
        Self::analysis_not_ready_error_static(&self.tracker, repo_id, original_error)
    }
}

#[tool_router]
impl CodeQueryTools {
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    /// Get schema information for a repository's code graph
    #[tool(
        description = "Get schema information for a repository's code graph (node types, edge types, and their properties). EXPERIMENTAL: This feature is under active development and may change. Useful for understanding the data model before writing custom SurrealQL queries with code_query."
    )]
    pub async fn describe_schema(
        &self,
        params: Parameters<DescribeSchemaParams>,
    ) -> Result<CallToolResult, McpError> {
        let repo_id = &params.0.repo_id;

        // Connect to the analysis database
        let graph =
            CodeGraph::with_connection_readonly(repo_id.clone(), Arc::clone(&self.analysis_db))
                .await
                .map_err(|e| self.analysis_not_ready_error(repo_id, &e.to_string()))?;

        // Get schema from SurrealDB
        let schema = graph.get_schema().await.map_err(|e| {
            McpError::internal_error(
                "schema_query_failed",
                Some(json!({
                    "message": format!("Failed to query schema: {}", e)
                })),
            )
        })?;

        let content = serde_json::to_string_pretty(&schema).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(json!({
                    "message": e.to_string()
                })),
            )
        })?;

        Ok(CallToolResult::success(vec![ContentBlock::text(content)]))
    }

    /// Execute queries (temporary or saved) against the code graph
    ///
    /// Supports 3 modes:
    /// 1. Temporary query (query_definition only) - execute without saving
    /// 2. Saved query (query_name only) - load from predefined or user-saved
    /// 3. Save and execute (both) - save to user directory then execute
    #[tool(
        description = "Execute queries against a repository's code graph. EXPERIMENTAL: This feature is under active development and may change. Use query_name for pre-built discovery queries (see code_list_queries), or query_definition for custom SurrealQL. The repo must be analyzed first (see code_analyze). Discovery workflow: 1) 'overview' for counts, 2) 'hub_symbols' or 'entry_points' for key code, 3) 'search_by_pattern' for concepts, 4) 'callers'/'callees' for dependencies, 5) 'find_tests_for' for coverage. All queries include usage examples in their descriptions."
    )]
    pub async fn query_graph(
        &self,
        params: Parameters<QueryCodeGraphParams>,
    ) -> Result<CallToolResult, McpError> {
        info!(
            "query_graph called: repo_id={}, query_name={:?}, has_definition={}",
            params.0.repo_id,
            params.0.query_name,
            params.0.query_definition.is_some()
        );

        // Validate: must have query_name OR query_definition
        if params.0.query_name.is_none() && params.0.query_definition.is_none() {
            return Err(McpError::invalid_params(
                "invalid_params",
                Some(json!({
                    "message": "Must provide either query_name or query_definition"
                })),
            ));
        }

        // Connect to analysis database
        let graph = CodeGraph::with_connection_readonly(
            params.0.repo_id.clone(),
            Arc::clone(&self.analysis_db),
        )
        .await
        .map_err(|e| self.analysis_not_ready_error(&params.0.repo_id, &e.to_string()))?;

        // Extract params as HashMap
        let query_params = params.0.variables.unwrap_or_default();

        // Determine query mode and load/save query SQL
        let (query_sql, query_type) = match (&params.0.query_name, &params.0.query_definition) {
            // Mode 1: Temporary query - execute directly without saving
            (None, Some(def)) => {
                debug!("Executing temporary query");
                (def.clone(), "temporary")
            }
            // Mode 2: Saved query - load from predefined or user-saved directory
            (Some(name), None) => {
                debug!("Loading saved query: {}", name);
                let query_sql = load_query(&graph, name)?;
                (query_sql, "saved")
            }
            // Mode 3: Save and execute - save to user directory then execute
            (Some(name), Some(def)) => {
                debug!("Saving query '{}' then executing", name);
                save_query(&graph, name, def)?;
                (def.clone(), "saved_and_executed")
            }
            (None, None) => unreachable!("Already validated above"),
        };

        // Execute query with auto-injected repo_id and user params
        let results = graph
            .execute_raw_query(&query_sql, query_params)
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "query_execution_failed",
                    Some(json!({
                        "message": format!("Query failed: {}", e)
                    })),
                )
            })?;

        info!(
            "Query completed successfully, returning {} results",
            results.len()
        );

        let response = json!({
            "query_type": query_type,
            "results": results,
        });

        let content = serde_json::to_string_pretty(&response).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(json!({
                    "message": e.to_string()
                })),
            )
        })?;

        Ok(CallToolResult::success(vec![ContentBlock::text(content)]))
    }

    /// List available queries for a repository's code graph
    ///
    /// Returns both predefined queries (from src/a6s/queries/) and user-saved queries
    #[tool(
        description = "List available pre-built discovery queries for a repository's code graph. EXPERIMENTAL: This feature is under active development and may change. Each query includes description, parameters, and concrete usage examples showing WHEN to use it. Queries are organized by discovery pattern: orientation (overview, hub_symbols, entry_points), search (search_by_pattern, symbol_search, explore_module), navigation (file_symbols, root_symbols, symbol_children), analysis (callers, callees, transitive_calls, data_flow), and testing (find_tests_for). Start with 'overview' to understand the codebase structure."
    )]
    pub async fn list_queries(
        &self,
        params: Parameters<ListQueriesParams>,
    ) -> Result<CallToolResult, McpError> {
        // Get predefined queries
        let predefined = CodeGraph::list_queries().map_err(|e| {
            McpError::internal_error(
                "fs_error",
                Some(json!({
                    "message": format!("Failed to list predefined queries: {}", e)
                })),
            )
        })?;

        // Connect to graph to get user-saved queries directory
        let graph = CodeGraph::with_connection_readonly(
            params.0.repo_id.clone(),
            Arc::clone(&self.analysis_db),
        )
        .await
        .map_err(|e| self.analysis_not_ready_error(&params.0.repo_id, &e.to_string()))?;

        let queries_dir = graph.get_queries_dir().map_err(|e| {
            McpError::internal_error(
                "fs_error",
                Some(json!({
                    "message": format!("Failed to get queries directory: {}", e)
                })),
            )
        })?;

        let mut user_saved = Vec::new();
        if queries_dir.exists() {
            let entries = std::fs::read_dir(&queries_dir).map_err(|e| {
                McpError::internal_error(
                    "fs_error",
                    Some(json!({
                        "message": format!("Failed to read queries directory: {}", e)
                    })),
                )
            })?;

            for entry in entries {
                let entry = entry.map_err(|e| {
                    McpError::internal_error(
                        "fs_error",
                        Some(json!({
                            "message": format!("Failed to read directory entry: {}", e)
                        })),
                    )
                })?;

                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("surql")
                    && let Some(name) = path.file_stem().and_then(|s| s.to_str())
                {
                    user_saved.push(name.to_string());
                }
            }
        }

        user_saved.sort();

        let response = json!({
            "repo_id": params.0.repo_id,
            "predefined_queries": predefined.iter().map(|q| json!({
                "name": q.name,
                "description": q.description,
                "params": q.params.iter().map(|p| json!({
                    "name": p.name,
                    "type": p.param_type,
                    "description": p.description,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "user_saved_queries": user_saved,
            "total": predefined.len() + user_saved.len(),
        });

        let content = serde_json::to_string_pretty(&response).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(json!({
                    "message": e.to_string()
                })),
            )
        })?;

        Ok(CallToolResult::success(vec![ContentBlock::text(content)]))
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Load a query by name from predefined or user-saved directory.
///
/// Checks predefined queries first, then falls back to user-saved.
fn load_query(graph: &CodeGraph, name: &str) -> Result<String, McpError> {
    // 1. Check predefined queries first (embedded at compile time)
    if let Some(query_sql) = crate::a6s::queries::PREDEFINED_QUERIES.get(name) {
        return Ok(query_sql.to_string());
    }

    // 2. Check user-saved queries
    let user_dir = graph.get_queries_dir().map_err(|e| {
        McpError::internal_error(
            "fs_error",
            Some(json!({
                "message": format!("Failed to get queries directory: {}", e)
            })),
        )
    })?;

    let user_saved = user_dir.join(format!("{}.surql", name));

    if user_saved.exists() {
        return std::fs::read_to_string(user_saved).map_err(|e| {
            McpError::internal_error(
                "fs_error",
                Some(json!({
                    "message": format!("Failed to read user-saved query: {}", e)
                })),
            )
        });
    }

    // Not found in either location
    let available: Vec<String> = CodeGraph::list_queries()
        .unwrap_or_default()
        .iter()
        .map(|q| q.name.clone())
        .collect();
    Err(McpError::invalid_params(
        "query_not_found",
        Some(json!({
            "message": format!("Query '{}' not found", name),
            "available_queries": available
        })),
    ))
}

/// Save a query to the user-saved queries directory.
fn save_query(graph: &CodeGraph, name: &str, query_sql: &str) -> Result<(), McpError> {
    let queries_dir = graph.get_queries_dir().map_err(|e| {
        McpError::internal_error(
            "fs_error",
            Some(json!({
                "message": format!("Failed to get queries directory: {}", e)
            })),
        )
    })?;

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&queries_dir).map_err(|e| {
        McpError::internal_error(
            "fs_error",
            Some(json!({
                "message": format!("Failed to create queries directory: {}", e)
            })),
        )
    })?;

    let query_file = queries_dir.join(format!("{}.surql", name));

    std::fs::write(&query_file, query_sql).map_err(|e| {
        McpError::internal_error(
            "fs_error",
            Some(json!({
                "message": format!("Failed to write query file: {}", e)
            })),
        )
    })?;

    Ok(())
}
