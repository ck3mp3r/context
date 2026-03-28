//! Code graph visualization endpoint.
//!
//! Serves graph data from NanoGraph in a format compatible with Sigma.js/Graphology.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

use crate::analysis::get_analysis_path;
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
    pub label: String,
    pub kind: String,
    pub language: String,
    pub file_path: String,
    pub start_line: i64,
    /// Node size computed from edge count
    pub size: f64,
    /// Color based on kind (Catppuccin palette)
    pub color: String,
    /// X position from layout
    pub x: f64,
    /// Y position from layout
    pub y: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    pub label: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GraphStats {
    pub total_symbols: usize,
    pub total_edges: usize,
    pub returned_nodes: usize,
    pub returned_edges: usize,
    pub truncated: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub stats: GraphStats,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct GraphQuery {
    /// Graph view: file-deps, calls, inherits, contains, full
    #[param(example = "full")]
    pub view: Option<String>,
    /// Max nodes to return (default 500, max 2000)
    #[param(example = 500)]
    pub limit: Option<usize>,
    /// Include test symbols (default false)
    #[param(example = false)]
    pub include_tests: Option<bool>,
    /// Filter by language (e.g. "rust", "nushell"). Omit for all languages.
    pub language: Option<String>,
}

// =============================================================================
// Color mapping (Catppuccin Mocha)
// =============================================================================

fn kind_color(kind: &str) -> &'static str {
    match kind {
        "function" | "method" | "command" => "#89b4fa", // blue
        "struct" => "#a6e3a1",                          // green
        "enum" => "#f9e2af",                            // yellow
        "trait" | "interface" => "#cba6f7",             // mauve
        "module" | "mod" => "#fab387",                  // peach
        "constant" | "const" => "#f2cdcd",              // flamingo
        "static" => "#f38ba8",                          // red
        "type_alias" | "type" => "#94e2d5",             // teal
        "macro" => "#f5c2e7",                           // pink
        "alias" => "#eba0ac",                           // maroon
        "extern" => "#74c7ec",                          // sapphire
        _ => "#a6adc8",                                 // subtext0 (fallback)
    }
}

// =============================================================================
// NanoGraph query execution
// =============================================================================

fn query_full_graph() -> &'static str {
    r#"query full_graph() {
    match {
        $s: Symbol
    }
    return {
        $s.symbol_id
        $s.name
        $s.kind
        $s.language
        $s.file_path
        $s.start_line
    }
}"#
}

fn query_calls_edges() -> &'static str {
    r#"query calls_edges() {
    match {
        $from: Symbol
        $to: Symbol
        $from calls $to
    }
    return {
        $from.symbol_id as source
        $to.symbol_id as target
    }
}"#
}

fn query_references_edges() -> &'static str {
    r#"query references_edges() {
    match {
        $from: Symbol
        $to: Symbol
        $from references $to
    }
    return {
        $from.symbol_id as source
        $to.symbol_id as target
    }
}"#
}

fn query_inherits_edges() -> &'static str {
    r#"query inherits_edges() {
    match {
        $from: Symbol
        $to: Symbol
        $from inherits $to
    }
    return {
        $from.symbol_id as source
        $to.symbol_id as target
    }
}"#
}

fn query_contains_edges() -> &'static str {
    r#"query contains_edges() {
    match {
        $parent: Symbol
        $child: Symbol
        $parent symbolContains $child
    }
    return {
        $parent.symbol_id as source
        $child.symbol_id as target
    }
}"#
}

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
/// Returns graph nodes and edges in a format compatible with Sigma.js/Graphology.
/// Returns 204 No Content if analysis has not been run for this repository.
#[utoipa::path(
    get,
    path = "/api/v1/repos/{id}/graph",
    tag = "repos",
    params(
        ("id" = String, Path, description = "Repo ID (8-character hex)"),
        GraphQuery,
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
    Query(query): Query<GraphQuery>,
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

    // Check if analysis exists - return 204 if not
    let analysis_dir = get_analysis_path(&id);
    let db_path = analysis_dir.join("analysis.nano");
    if !db_path.exists() {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    let limit = query.limit.unwrap_or(500).min(2000);
    let view = query.view.as_deref().unwrap_or("full");
    let include_tests = query.include_tests.unwrap_or(false);
    let language_filter = query.language.clone();

    // Run queries in a blocking task (shells out to nanograph CLI)
    let db_path_clone = db_path.clone();
    let view_owned = view.to_string();

    let result = tokio::task::spawn_blocking(move || {
        build_graph_data(
            &db_path_clone,
            &view_owned,
            limit,
            include_tests,
            language_filter.as_deref(),
        )
    })
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

fn is_test_symbol(s: &serde_json::Value) -> bool {
    let file_path = s["file_path"].as_str().unwrap_or("");
    let name = s["name"].as_str().unwrap_or("");
    // Rust test files/modules
    file_path.contains("_test.rs")
        || file_path.contains("/tests/")
        || file_path.contains("/test/")
        || name.starts_with("test_")
        || name == "tests"
}

fn build_graph_data(
    db_path: &std::path::Path,
    view: &str,
    limit: usize,
    include_tests: bool,
    language_filter: Option<&str>,
) -> Result<GraphResponse, String> {
    // Fetch all symbols
    let all_symbols = run_nanograph_query(db_path, query_full_graph(), "full_graph")?;
    let total_symbols = all_symbols.len();

    // Filter out test symbols unless include_tests is set
    let symbols: Vec<serde_json::Value> = all_symbols
        .into_iter()
        .filter(|s| include_tests || !is_test_symbol(s))
        .filter(|s| {
            language_filter
                .map(|lang| s["language"].as_str().unwrap_or("") == lang)
                .unwrap_or(true)
        })
        .collect();

    // Determine which edge types to fetch based on view
    let edge_queries: Vec<(&str, &str, &str)> = match view {
        "calls" => vec![("calls_edges", "Calls", query_calls_edges())],
        "inherits" => vec![("inherits_edges", "Inherits", query_inherits_edges())],
        "contains" => vec![("contains_edges", "Contains", query_contains_edges())],
        "references" => vec![("references_edges", "References", query_references_edges())],
        _ => vec![
            ("calls_edges", "Calls", query_calls_edges()),
            ("references_edges", "References", query_references_edges()),
            ("inherits_edges", "Inherits", query_inherits_edges()),
            ("contains_edges", "Contains", query_contains_edges()),
        ],
    };

    // Fetch edges
    let mut all_edges: Vec<(String, String, String)> = Vec::new();
    for (query_name, edge_type, query_content) in &edge_queries {
        let edges = run_nanograph_query(db_path, query_content, query_name)?;
        for edge in edges {
            let source = edge["source"].as_str().unwrap_or("").to_string();
            let target = edge["target"].as_str().unwrap_or("").to_string();
            if !source.is_empty() && !target.is_empty() {
                all_edges.push((source, target, edge_type.to_string()));
            }
        }
    }
    let total_edges = all_edges.len();

    // Build node set (truncate to limit)
    let truncated = symbols.len() > limit;
    let symbols_limited: Vec<_> = symbols.into_iter().take(limit).collect();

    // Collect node IDs for filtering edges
    let node_ids: std::collections::HashSet<String> = symbols_limited
        .iter()
        .filter_map(|s| s["symbol_id"].as_str().map(String::from))
        .collect();

    // Count edges per node for sizing
    let mut edge_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (source, target, _) in &all_edges {
        if node_ids.contains(source) {
            *edge_counts.entry(source.clone()).or_default() += 1;
        }
        if node_ids.contains(target) {
            *edge_counts.entry(target.clone()).or_default() += 1;
        }
    }

    // Build nodes (without positions yet)
    let mut nodes: Vec<GraphNode> = symbols_limited
        .iter()
        .filter_map(|s| {
            let id = s["symbol_id"].as_str()?.to_string();
            let kind = s["kind"].as_str().unwrap_or("unknown");
            let language = s["language"].as_str().unwrap_or("unknown").to_string();
            let degree = *edge_counts.get(&id).unwrap_or(&0);
            Some(GraphNode {
                label: s["name"].as_str().unwrap_or("?").to_string(),
                kind: kind.to_string(),
                language,
                file_path: s["file_path"].as_str().unwrap_or("").to_string(),
                start_line: s["start_line"].as_i64().unwrap_or(0),
                size: 3.0 + (degree as f64).sqrt() * 2.0,
                color: kind_color(kind).to_string(),
                id,
                x: 0.0,
                y: 0.0,
            })
        })
        .collect();

    // Filter edges to only include edges between visible nodes
    let edges: Vec<GraphEdge> = all_edges
        .into_iter()
        .enumerate()
        .filter(|(_, (source, target, _))| node_ids.contains(source) && node_ids.contains(target))
        .map(|(i, (source, target, edge_type))| {
            let label = edge_type.to_lowercase();
            GraphEdge {
                id: format!("e{}", i),
                source,
                target,
                label,
                edge_type,
            }
        })
        .collect();

    // Compute force-directed layout
    compute_layout(&mut nodes, &edges);

    Ok(GraphResponse {
        stats: GraphStats {
            total_symbols,
            total_edges,
            returned_nodes: nodes.len(),
            returned_edges: edges.len(),
            truncated,
        },
        nodes,
        edges,
    })
}

// =============================================================================
// Force-directed layout (Fruchterman-Reingold)
// =============================================================================

fn compute_layout(nodes: &mut [GraphNode], edges: &[GraphEdge]) {
    let n = nodes.len();
    if n == 0 {
        return;
    }
    if n == 1 {
        nodes[0].x = 0.0;
        nodes[0].y = 0.0;
        return;
    }

    // Build index map: node_id -> position in nodes array
    let id_to_idx: std::collections::HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Build adjacency list for neighbor lookups
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
    let edge_pairs: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|e| {
            let s = *id_to_idx.get(e.source.as_str())?;
            let t = *id_to_idx.get(e.target.as_str())?;
            neighbors[s].push(t);
            neighbors[t].push(s);
            Some((s, t))
        })
        .collect();

    // --- Parameters ---
    // k = optimal edge length, scale with sqrt(area_per_node)
    let spread = 10.0; // tuning: higher = more spread out
    let k = spread * (1000.0 / n as f64).sqrt();
    let k2 = k * k;

    // --- Initial positions: group by parent directory ---
    // Nodes from the same directory start near each other, giving the
    // algorithm a massive head start vs random placement.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Assign each unique parent dir an angle on a circle
    let mut dir_angles: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for node in nodes.iter() {
        let dir = node
            .file_path
            .rfind('/')
            .map(|i| &node.file_path[..i])
            .unwrap_or("")
            .to_string();
        let count = dir_angles.len();
        dir_angles.entry(dir).or_insert_with(|| count as f64);
    }
    let num_dirs = dir_angles.len().max(1) as f64;
    // Convert indices to angles
    for angle in dir_angles.values_mut() {
        *angle = *angle / num_dirs * std::f64::consts::TAU;
    }

    let cluster_radius = k * (n as f64).sqrt() * 0.3;
    let jitter_radius = k * 2.0;
    for (i, node) in nodes.iter_mut().enumerate() {
        let dir = node
            .file_path
            .rfind('/')
            .map(|idx| &node.file_path[..idx])
            .unwrap_or("");
        let base_angle = dir_angles.get(dir).copied().unwrap_or(0.0);
        // Deterministic jitter from node id
        let mut hasher = DefaultHasher::new();
        node.id.hash(&mut hasher);
        i.hash(&mut hasher);
        let h = hasher.finish();
        let jx = ((h & 0xFFFF) as f64 / 65535.0 - 0.5) * jitter_radius;
        let jy = (((h >> 16) & 0xFFFF) as f64 / 65535.0 - 0.5) * jitter_radius;
        node.x = base_angle.cos() * cluster_radius + jx;
        node.y = base_angle.sin() * cluster_radius + jy;
    }

    // --- Iteration parameters ---
    let iterations = 200.min(100 + n / 3);
    let initial_temp = k * (n as f64).sqrt() * 0.1;
    let mut temperature = initial_temp;
    // Geometric cooling: temp *= cooling_factor each iteration
    let cooling_factor = (0.01_f64).powf(1.0 / iterations as f64);

    let mut dx = vec![0.0f64; n];
    let mut dy = vec![0.0f64; n];

    for _ in 0..iterations {
        // Reset displacements
        for d in dx.iter_mut() {
            *d = 0.0;
        }
        for d in dy.iter_mut() {
            *d = 0.0;
        }

        // Repulsive forces (all pairs) — FR: k²/d
        for i in 0..n {
            for j in (i + 1)..n {
                let ddx = nodes[i].x - nodes[j].x;
                let ddy = nodes[i].y - nodes[j].y;
                let dist2 = (ddx * ddx + ddy * ddy).max(0.001);
                // Force magnitude = k²/d = k²/sqrt(dist2)
                // Component = (ddx/d) * k²/d = ddx * k² / dist2
                let factor = k2 / dist2;
                let fx = ddx * factor;
                let fy = ddy * factor;
                dx[i] += fx;
                dy[i] += fy;
                dx[j] -= fx;
                dy[j] -= fy;
            }
        }

        // Attractive forces (edges only) — FR: d²/k
        for &(si, ti) in &edge_pairs {
            let ddx = nodes[si].x - nodes[ti].x;
            let ddy = nodes[si].y - nodes[ti].y;
            let dist = (ddx * ddx + ddy * ddy).sqrt().max(0.001);
            // Force = d/k (linear attractive, classic FR)
            let force = dist / k;
            let fx = (ddx / dist) * force;
            let fy = (ddy / dist) * force;
            dx[si] -= fx;
            dy[si] -= fy;
            dx[ti] += fx;
            dy[ti] += fy;
        }

        // Gentle gravity: prevent disconnected components from drifting
        // Force proportional to distance from center (not node size)
        let gravity = 0.1;
        for i in 0..n {
            dx[i] -= gravity * nodes[i].x;
            dy[i] -= gravity * nodes[i].y;
        }

        // Apply displacements clamped by temperature
        for i in 0..n {
            let disp = (dx[i] * dx[i] + dy[i] * dy[i]).sqrt().max(0.001);
            let capped = disp.min(temperature);
            nodes[i].x += (dx[i] / disp) * capped;
            nodes[i].y += (dy[i] / disp) * capped;
        }

        temperature *= cooling_factor;
    }
}
