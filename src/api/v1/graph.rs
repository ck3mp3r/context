//! Code graph visualization endpoint.
//!
//! Returns all symbols and edges from SurrealDB. Layout, filtering, and
//! truncation are handled entirely by the frontend (Sigma.js / Graphology).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;
use std::collections::HashMap;
use tracing::instrument;
use utoipa::ToSchema;

use crate::a6s::store::CodeGraph;
use crate::api::AppState;
use crate::db::{Database, RepoRepository};
use crate::sync::GitOps;

use super::ErrorResponse;

// =============================================================================
// DTOs
// =============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct GraphNode {
    pub id: String,
    /// Short display label (bare symbol name)
    pub label: String,
    /// Module-qualified name (e.g., "analysis::types::SymbolId")
    pub qualified_name: String,
    pub kind: String,
    pub language: String,
    pub file_path: String,
    pub start_line: i64,
    /// Entry point type: "main", "test", "export", "init", "benchmark", "example", or null
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_type: Option<String>,
    /// Deprecated: use entry_type == "test" instead
    pub is_test: bool,
    /// DEBUG: module_path from database
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_path: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GraphStats {
    pub total_symbols: usize,
    pub total_edges: usize,
    /// Names of queries that failed during graph construction.
    /// Empty if all queries succeeded.
    pub failed_queries: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub stats: GraphStats,
}

// =============================================================================
// Handler
// =============================================================================

/// Get code graph data for visualization
///
/// Returns all symbols and edges from the analysis graph. Layout, filtering,
/// and drill-down are handled client-side by Sigma.js / Graphology.
/// Returns 204 No Content if analysis has not been run for this repository.
#[utoipa::path(
    get,
    path = "/api/v1/repos/{id}/graph",
    tag = "repos",
    params(
        ("id" = String, Path, description = "Repo ID (8-character hex)"),
    ),
    responses(
        (status = 200, description = "Graph data", body = GraphResponse),
        (status = 204, description = "No analysis available for this repo"),
        (status = 404, description = "Repo not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn get_repo_graph<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Path(id): Path<String>,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    // Verify repo exists
    state.db().repos().get(&id).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Repo '{}' not found", id),
            }),
        )
    })?;

    // Use shared database connection from AppState to avoid RocksDB lock contention
    // Note: We DON'T call CodeGraph::new() because that truncates the repo!
    // Instead, we create a CodeGraph without truncation for read-only access
    let graph = CodeGraph::with_connection_readonly(id.clone(), state.analysis_db())
        .await
        .map_err(|e| {
            tracing::warn!("Failed to access analysis for repo {}: {}", id, e);
            // Return 204 No Content if analysis doesn't exist
            (
                StatusCode::NO_CONTENT,
                Json(ErrorResponse {
                    error: format!("No analysis available for repository {}", id),
                }),
            )
        })?;

    // Build graph data from SurrealDB
    let result = build_graph_data(&graph).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e }),
        )
    })?;

    Ok(Json(result).into_response())
}

async fn build_graph_data(graph: &CodeGraph) -> Result<GraphResponse, String> {
    let mut all_edges: Vec<(String, String, String)> = Vec::new();
    let mut symbol_map: HashMap<String, serde_json::Value> = HashMap::new();
    let mut failed_queries: Vec<String> = Vec::new();

    // Query ALL symbols first
    let all_symbols = match graph.execute_query("all_symbols", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("all_symbols query failed: {}", e);
            failed_queries.push("all_symbols".to_string());
            Vec::new()
        }
    };

    for row in all_symbols {
        let symbol_id = row["symbol_id"].as_str().unwrap_or("").to_string();
        if symbol_id.is_empty() {
            continue;
        }
        symbol_map.insert(symbol_id, row);
    }

    // Query for Calls edges
    let calls_rows = match graph.execute_query("calls_edges", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("calls_edges query failed: {}", e);
            failed_queries.push("calls_edges".to_string());
            Vec::new()
        }
    };
    for row in calls_rows {
        let src_id = row["src_id"].as_str().unwrap_or("").to_string();
        let dst_id = row["dst_id"].as_str().unwrap_or("").to_string();
        if src_id.is_empty() || dst_id.is_empty() {
            continue;
        }
        all_edges.push((src_id, dst_id, "Calls".to_string()));
    }

    // Query for FileImports edges
    let file_import_rows = match graph.execute_query("file_imports", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("file_imports query failed: {}", e);
            failed_queries.push("file_imports".to_string());
            Vec::new()
        }
    };
    for row in file_import_rows {
        let src_id = row["src_id"].as_str().unwrap_or("").to_string();
        let dst_id = row["dst_id"].as_str().unwrap_or("").to_string();
        if src_id.is_empty() || dst_id.is_empty() {
            continue;
        }
        all_edges.push((src_id, dst_id, "FileImports".to_string()));
    }

    // Query for HasField edges
    let field_rows = match graph.execute_query("has_field", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("has_field query failed: {}", e);
            failed_queries.push("has_field".to_string());
            Vec::new()
        }
    };
    for row in field_rows {
        let src_id = row["src_id"].as_str().unwrap_or("").to_string();
        let dst_id = row["dst_id"].as_str().unwrap_or("").to_string();
        if src_id.is_empty() || dst_id.is_empty() {
            continue;
        }
        all_edges.push((src_id, dst_id, "HasField".to_string()));
    }

    // Query for HasMethod edges
    let method_rows = match graph.execute_query("has_method", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("has_method query failed: {}", e);
            failed_queries.push("has_method".to_string());
            Vec::new()
        }
    };
    for row in method_rows {
        let src_id = row["src_id"].as_str().unwrap_or("").to_string();
        let dst_id = row["dst_id"].as_str().unwrap_or("").to_string();
        if src_id.is_empty() || dst_id.is_empty() {
            continue;
        }
        all_edges.push((src_id, dst_id, "HasMethod".to_string()));
    }

    // Query for HasMember edges
    let member_rows = match graph.execute_query("has_member", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("has_member query failed: {}", e);
            failed_queries.push("has_member".to_string());
            Vec::new()
        }
    };
    for row in member_rows {
        let src_id = row["src_id"].as_str().unwrap_or("").to_string();
        let dst_id = row["dst_id"].as_str().unwrap_or("").to_string();
        if src_id.is_empty() || dst_id.is_empty() {
            continue;
        }
        all_edges.push((src_id, dst_id, "HasMember".to_string()));
    }

    // Query for Implements edges
    let implements_rows = match graph.execute_query("implements", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("implements query failed: {}", e);
            failed_queries.push("implements".to_string());
            Vec::new()
        }
    };
    for row in implements_rows {
        let src_id = row["src_id"].as_str().unwrap_or("").to_string();
        let dst_id = row["dst_id"].as_str().unwrap_or("").to_string();
        if src_id.is_empty() || dst_id.is_empty() {
            continue;
        }
        all_edges.push((src_id, dst_id, "Implements".to_string()));
    }

    // Query for Extends edges
    let extends_rows = match graph.execute_query("extends", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("extends query failed: {}", e);
            failed_queries.push("extends".to_string());
            Vec::new()
        }
    };
    for row in extends_rows {
        let src_id = row["src_id"].as_str().unwrap_or("").to_string();
        let dst_id = row["dst_id"].as_str().unwrap_or("").to_string();
        if src_id.is_empty() || dst_id.is_empty() {
            continue;
        }
        all_edges.push((src_id, dst_id, "Extends".to_string()));
    }

    // Query for Accepts edges (parameter types)
    let accepts_rows = match graph.execute_query("accepts_edges", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("accepts_edges query failed: {}", e);
            failed_queries.push("accepts_edges".to_string());
            Vec::new()
        }
    };
    for row in accepts_rows {
        let src_id = row["src_id"].as_str().unwrap_or("").to_string();
        let dst_id = row["dst_id"].as_str().unwrap_or("").to_string();
        if src_id.is_empty() || dst_id.is_empty() {
            continue;
        }
        all_edges.push((src_id, dst_id, "Accepts".to_string()));
    }

    // Query for Returns edges (return types)
    let returns_rows = match graph.execute_query("returns_edges", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("returns_edges query failed: {}", e);
            failed_queries.push("returns_edges".to_string());
            Vec::new()
        }
    };
    for row in returns_rows {
        let src_id = row["src_id"].as_str().unwrap_or("").to_string();
        let dst_id = row["dst_id"].as_str().unwrap_or("").to_string();
        if src_id.is_empty() || dst_id.is_empty() {
            continue;
        }
        all_edges.push((src_id, dst_id, "Returns".to_string()));
    }

    // Query for FieldType edges
    let field_type_rows = match graph
        .execute_query("field_type_edges", HashMap::new())
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("field_type_edges query failed: {}", e);
            failed_queries.push("field_type_edges".to_string());
            Vec::new()
        }
    };
    for row in field_type_rows {
        let src_id = row["src_id"].as_str().unwrap_or("").to_string();
        let dst_id = row["dst_id"].as_str().unwrap_or("").to_string();
        if src_id.is_empty() || dst_id.is_empty() {
            continue;
        }
        all_edges.push((src_id, dst_id, "FieldType".to_string()));
    }

    let total_symbols = symbol_map.len();
    let total_edges = all_edges.len();

    let nodes: Vec<GraphNode> = symbol_map
        .into_iter()
        .filter_map(|(_symbol_id, s)| {
            let id = s["symbol_id"].as_str()?.to_string();
            let kind = s["kind"].as_str().unwrap_or("unknown");
            let name = s["name"].as_str().unwrap_or("?");
            let file_path = s["file_path"].as_str().unwrap_or("");
            let language = s["language"].as_str().unwrap_or("unknown");
            let entry_type = s["entry_type"].as_str().unwrap_or("");

            // Use module_path from database if available, otherwise construct it
            let module_path_str = s["module_path"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_default();

            let qualified_name = if module_path_str.is_empty() {
                name.to_string()
            } else {
                format!("{}::{}", module_path_str, name)
            };

            Some(GraphNode {
                id,
                label: name.to_string(),
                qualified_name,
                kind: kind.to_string(),
                language: language.to_string(),
                file_path: file_path.to_string(),
                start_line: s["start_line"].as_i64().unwrap_or(0),
                entry_type: if entry_type.is_empty() {
                    None
                } else {
                    Some(entry_type.to_string())
                },
                is_test: entry_type == "test",
                module_path: if module_path_str.is_empty() {
                    None
                } else {
                    Some(module_path_str)
                },
            })
        })
        .collect();

    let edges: Vec<GraphEdge> = all_edges
        .into_iter()
        .enumerate()
        .map(|(i, (source, target, edge_type))| GraphEdge {
            id: format!("e{}", i),
            source,
            target,
            edge_type,
        })
        .collect();

    Ok(GraphResponse {
        stats: GraphStats {
            total_symbols,
            total_edges,
            failed_queries,
        },
        nodes,
        edges,
    })
}
