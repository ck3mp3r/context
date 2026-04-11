//! Code graph visualization endpoint.
//!
//! Returns all symbols and edges from NanoGraph. Layout, filtering, and
//! truncation are handled entirely by the frontend (Sigma.js / Graphology).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;
use std::process::Command;
use tracing::instrument;
use utoipa::ToSchema;

use crate::a6s::queries;
use crate::analysis::get_analysis_path;
use crate::analysis::lang::{Analyser, LanguageAnalyser};
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
// NanoGraph query helpers
// =============================================================================

fn run_nanograph_query(
    db_path: &std::path::Path,
    query_content: &str,
    query_name: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let query_file = db_path.join(format!("_temp_{}.gq", query_name));
    std::fs::write(&query_file, query_content)
        .map_err(|e| format!("Failed to write query file: {}", e))?;

    let output = Command::new("nanograph")
        .arg("run")
        .arg("--db")
        .arg(db_path)
        .arg("--query")
        .arg(&query_file)
        .arg("--name")
        .arg(query_name)
        .arg("--format")
        .arg("jsonl")
        .output()
        .map_err(|e| format!("Failed to run nanograph: {}", e))?;

    let _ = std::fs::remove_file(&query_file);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("nanograph query failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let obj: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("JSON parse error: {}", e))?;
        results.push(obj);
    }

    Ok(results)
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
    state.db().repos().get(&id).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Repo '{}' not found", id),
            }),
        )
    })?;

    let analysis_dir = get_analysis_path(&id);
    let db_path = analysis_dir.join("analysis.nano");
    if !db_path.exists() {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    let result = tokio::task::spawn_blocking(move || build_graph_data(&db_path))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Task join error: {}", e),
                }),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e }),
            )
        })?;

    Ok(Json(result).into_response())
}

fn build_graph_data(db_path: &std::path::Path) -> Result<GraphResponse, String> {
    let mut all_edges: Vec<(String, String, String)> = Vec::new();
    let mut symbol_map: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    let mut failed_queries: Vec<String> = Vec::new();

    // Query ALL symbols first
    let all_symbols = match run_nanograph_query(db_path, queries::ALL_SYMBOLS, "all_symbols") {
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
        let sym = serde_json::json!({
            "symbol_id": row["symbol_id"],
            "name": row["name"],
            "kind": row["kind"],
            "language": row["language"],
            "file_path": row["file_path"],
            "start_line": row["start_line"],
            "entry_type": row["entry_type"],
        });
        symbol_map.insert(symbol_id, sym);
    }

    // Query for Calls edges using pre-loaded query
    let calls_rows = match run_nanograph_query(db_path, queries::CALLS_EDGES, "calls") {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("calls query failed: {}", e);
            failed_queries.push("calls".to_string());
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

    // Query for FileImports edges using pre-loaded query
    let file_import_rows = match run_nanograph_query(db_path, queries::FILE_IMPORTS, "fileimports")
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("fileimports query failed: {}", e);
            failed_queries.push("fileimports".to_string());
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
    let field_rows = match run_nanograph_query(db_path, queries::HAS_FIELD, "hasfield") {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("hasfield query failed: {}", e);
            failed_queries.push("hasfield".to_string());
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
    let method_rows = match run_nanograph_query(db_path, queries::HAS_METHOD, "hasmethod") {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("hasmethod query failed: {}", e);
            failed_queries.push("hasmethod".to_string());
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
    let member_rows = match run_nanograph_query(db_path, queries::HAS_MEMBER, "hasmember") {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("hasmember query failed: {}", e);
            failed_queries.push("hasmember".to_string());
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
    let implements_rows = match run_nanograph_query(db_path, queries::IMPLEMENTS, "implements") {
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
    let extends_rows = match run_nanograph_query(db_path, queries::EXTENDS, "extends") {
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

            let module_path = Analyser::for_language(language)
                .map(|a| a.derive_module_path(file_path))
                .unwrap_or_default();

            let qualified_name = if module_path.is_empty() {
                name.to_string()
            } else {
                format!("{}::{}", module_path, name)
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
