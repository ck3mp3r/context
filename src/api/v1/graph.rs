//! Code graph visualization endpoint.
//!
//! Returns all symbols and edges from SurrealDB. Layout, filtering, and
//! truncation are handled entirely by the frontend (Sigma.js / Graphology).

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

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
    /// Number of direct children via containment edges (has_member, has_method, has_field)
    pub child_count: u32,
    /// Parent symbol ID if this is a child via containment edge
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    /// Aggregate edge counts by edge type (only present for aggregate edges)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate_counts: Option<HashMap<String, u32>>,
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

#[derive(Debug, Deserialize, IntoParams)]
pub struct GraphQuery {
    /// Root symbol ID for subtree view (optional)
    #[param(example = "abc123")]
    pub root: Option<String>,
    /// Depth of children to expand (default: 1, max: 5)
    #[param(example = 1)]
    pub depth: Option<u32>,
    /// Comma-separated list of visible symbol IDs for filtering edges (optional)
    #[param(example = "abc123,def456")]
    pub visible_ids: Option<String>,
    /// Search term for symbol name or qualified_name (case-insensitive, returns matches with ancestor chains)
    #[param(example = "parse")]
    pub search: Option<String>,
}

// =============================================================================
// Handler
// =============================================================================

/// Get code graph data for visualization
///
/// Returns all symbols and edges from the analysis graph. Layout, filtering,
/// and drill-down are handled client-side by Sigma.js / Graphology.
/// Returns 204 No Content if analysis has not been run for this repository.
///
/// Query parameters:
/// - `root`: Root symbol ID for subtree view (returns children of this symbol)
/// - `depth`: How many levels of children to expand (default: 1, max: 5)
/// - `visible_ids`: Comma-separated list of symbol IDs for edge filtering
/// - `search`: Search term for symbol names (case-insensitive). Returns matching symbols with their ancestor chains.
///
/// If no `root` is provided, returns root symbols (symbols with no parent container).
/// If `search` is provided, ignores other parameters and returns search results with ancestor chains.
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
    let result = if let Some(search_term) = &query.search {
        // Search mode - return matching symbols with ancestor chains
        build_search_results(&graph, search_term).await
    } else if let Some(root_id) = &query.root {
        // Root specified - return subtree with children
        let depth = query.depth.unwrap_or(1).min(5); // Cap at 5
        let visible_ids = query.visible_ids.as_ref().map(|ids| {
            ids.split(',')
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>()
        });
        build_subtree_data(&graph, root_id, depth, visible_ids).await
    } else {
        // No root specified - return root symbols and edges between them
        build_root_graph_data(&graph).await
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e }),
        )
    })?;

    Ok(Json(result).into_response())
}

/// Edge query definitions: (query_name, edge_type_label)
const EDGE_QUERIES: &[(&str, &str)] = &[
    ("calls_edges", "Calls"),
    ("implements", "Implements"),
    ("extends", "Extends"),
    ("accepts_edges", "Accepts"),
    ("returns_edges", "Returns"),
    ("field_type_edges", "FieldType"),
    ("annotates_type", "TypeAnnotation"),
    ("uses_type", "Uses"),
];

/// Containment edge query names
const CONTAINMENT_QUERIES: &[&str] = &["declares_mod", "has_member", "has_method", "has_field"];

/// Fetch all non-containment edges for the repository.
/// Returns (src_id, dst_id, edge_type) tuples.
/// This function fetches ALL edges and relies on Rust filtering for performance
/// instead of SurrealDB's slow IN $array checks.
async fn fetch_non_containment_edges(
    graph: &CodeGraph,
    failed_queries: &mut Vec<String>,
) -> Vec<(String, String, String)> {
    let mut all_edges = Vec::new();

    for (query_name, edge_type) in EDGE_QUERIES {
        match graph.execute_query(query_name, HashMap::new()).await {
            Ok(rows) => {
                for row in rows {
                    if let (Some(src), Some(dst)) = (row["src_id"].as_str(), row["dst_id"].as_str())
                    {
                        all_edges.push((src.to_string(), dst.to_string(), edge_type.to_string()));
                    }
                }
            }
            Err(e) => {
                tracing::warn!("{} query failed: {}", query_name, e);
                failed_queries.push(query_name.to_string());
            }
        }
    }

    all_edges
}

/// Fetch containment edges (has_member, has_method, has_field).
/// Returns:
/// - child_ids: Set of all child symbol IDs
/// - child_counts: Map of parent_id -> count of direct children
/// - containment_edges: Map of parent_id -> list of (child_id, edge_type) tuples
/// - failed_queries: Names of queries that failed
async fn fetch_containment_data(
    graph: &CodeGraph,
) -> (
    HashSet<String>,
    HashMap<String, u32>,
    HashMap<String, Vec<(String, String)>>,
    Vec<String>,
) {
    let mut child_ids: HashSet<String> = HashSet::new();
    let mut child_counts: HashMap<String, u32> = HashMap::new();
    let mut containment_edges: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut failed_queries: Vec<String> = Vec::new();

    for query_name in CONTAINMENT_QUERIES {
        // Map query name to edge type label
        let edge_type = match *query_name {
            "has_member" => "HasMember",
            "has_method" => "HasMethod",
            "has_field" => "HasField",
            _ => "Contains",
        };

        match graph.execute_query(query_name, HashMap::new()).await {
            Ok(rows) => {
                for row in rows {
                    if let (Some(src), Some(dst)) = (row["src_id"].as_str(), row["dst_id"].as_str())
                    {
                        child_ids.insert(dst.to_string());
                        *child_counts.entry(src.to_string()).or_insert(0) += 1;
                        containment_edges
                            .entry(src.to_string())
                            .or_default()
                            .push((dst.to_string(), edge_type.to_string()));
                    }
                }
            }
            Err(e) => {
                tracing::warn!("{} query failed: {}", query_name, e);
                failed_queries.push(query_name.to_string());
            }
        }
    }

    (child_ids, child_counts, containment_edges, failed_queries)
}

/// Build child-to-root mapping by walking parent chains.
/// Returns a map from each symbol ID to its root ancestor.
/// Public for testing.
pub fn build_child_to_root_map(
    symbol_ids: &[String],
    parent_map: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut child_to_root: HashMap<String, String> = HashMap::new();

    // Walk up to root for each symbol
    for symbol_id in symbol_ids {
        let mut current = symbol_id.clone();
        let mut visited: HashSet<String> = HashSet::new();

        // Walk up parent chain until we find a root (no parent)
        while let Some(parent) = parent_map.get(&current) {
            if visited.contains(parent) {
                // Cycle detected - treat current as root
                break;
            }
            visited.insert(current.clone());
            current = parent.clone();
        }

        // current is now the root
        child_to_root.insert(symbol_id.clone(), current);
    }

    child_to_root
}

/// Compute aggregate edges from non-containment edges.
/// Returns list of (root_a, root_b, edge_counts) tuples.
/// Self-loops are excluded. Bidirectional edges are merged (A-B and B-A become one edge).
/// Public for testing.
pub fn compute_aggregate_edges(
    child_to_root: &HashMap<String, String>,
    non_containment_edges: &[(String, String, String)],
) -> Vec<(String, String, HashMap<String, u32>)> {
    // Map: (root_a, root_b, edge_type) -> count
    let mut aggregate_map: HashMap<(String, String, String), u32> = HashMap::new();

    for (src, dst, edge_type) in non_containment_edges {
        // Resolve source and destination to their root ancestors
        let src_root = child_to_root
            .get(src)
            .cloned()
            .unwrap_or_else(|| src.clone());
        let dst_root = child_to_root
            .get(dst)
            .cloned()
            .unwrap_or_else(|| dst.clone());

        // Skip self-loops
        if src_root == dst_root {
            continue;
        }

        // Sort the pair to ensure (A, B) and (B, A) map to the same edge
        let (root_a, root_b) = if src_root < dst_root {
            (src_root, dst_root)
        } else {
            (dst_root, src_root)
        };

        *aggregate_map
            .entry((root_a, root_b, edge_type.clone()))
            .or_insert(0) += 1;
    }

    // Group by (root_a, root_b) and collect edge types
    let mut grouped: HashMap<(String, String), HashMap<String, u32>> = HashMap::new();
    for ((root_a, root_b, edge_type), count) in aggregate_map {
        grouped
            .entry((root_a, root_b))
            .or_default()
            .insert(edge_type, count);
    }

    // Convert to edge list with aggregate_counts
    grouped
        .into_iter()
        .map(|((root_a, root_b), counts)| (root_a, root_b, counts))
        .collect()
}

async fn build_root_graph_data(graph: &CodeGraph) -> Result<GraphResponse, String> {
    let mut symbol_map: HashMap<String, serde_json::Value> = HashMap::new();
    let mut failed_queries: Vec<String> = Vec::new();

    // Step 1: Fetch ALL symbols
    let all_symbols = match graph.execute_query("all_symbols", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("all_symbols query failed: {}", e);
            failed_queries.push("all_symbols".to_string());
            Vec::new()
        }
    };

    // Build initial symbol lookup (symbol_id -> symbol_data)
    let mut all_symbols_map: HashMap<String, serde_json::Value> = HashMap::new();
    for row in all_symbols {
        let symbol_id = row["symbol_id"].as_str().unwrap_or("").to_string();
        if symbol_id.is_empty() {
            continue;
        }
        all_symbols_map.insert(symbol_id, row);
    }

    // Step 2: Fetch containment edges (has_member, has_method, has_field)
    let (child_ids, child_counts, containment_edges, containment_failures) =
        fetch_containment_data(graph).await;
    failed_queries.extend(containment_failures);

    // Step 3: Build parent map from containment edges (target -> source, since source is parent)
    let mut parent_map: HashMap<String, String> = HashMap::new();
    for (parent_id, children) in &containment_edges {
        for (child_id, _edge_type) in children {
            parent_map.insert(child_id.clone(), parent_id.clone());
        }
    }

    // Step 4: Identify root symbols (not in child_ids) and inject child_count and parent_id
    let mut root_ids: Vec<String> = Vec::new();
    for (symbol_id, symbol_value) in &all_symbols_map {
        if !child_ids.contains(symbol_id) {
            // This is a root symbol
            let mut symbol_value = symbol_value.clone();
            if let Some(obj) = symbol_value.as_object_mut() {
                let count = child_counts.get(symbol_id).copied().unwrap_or(0);
                obj.insert("child_count".to_string(), serde_json::Value::from(count));
                obj.insert("parent_id".to_string(), serde_json::Value::Null);
            }
            symbol_map.insert(symbol_id.clone(), symbol_value);
            root_ids.push(symbol_id.clone());
        }
    }

    // Step 5: Auto-expand root nodes' direct children (depth 1 BFS)
    let mut containment_edge_list: Vec<(String, String, String)> = Vec::new();
    for root_id in &root_ids {
        if let Some(children) = containment_edges.get(root_id) {
            for (child_id, edge_type) in children {
                // Add child if not already in map
                if !symbol_map.contains_key(child_id)
                    && let Some(child_data) = all_symbols_map.get(child_id)
                {
                    let mut child_data = child_data.clone();
                    // Inject child_count and parent_id
                    if let Some(obj) = child_data.as_object_mut() {
                        let count = child_counts.get(child_id).copied().unwrap_or(0);
                        obj.insert("child_count".to_string(), serde_json::Value::from(count));
                        obj.insert(
                            "parent_id".to_string(),
                            serde_json::Value::String(root_id.clone()),
                        );
                    }
                    symbol_map.insert(child_id.clone(), child_data);
                }

                // Add containment edge if child is in symbol_map
                if symbol_map.contains_key(child_id) {
                    containment_edge_list.push((
                        root_id.clone(),
                        child_id.clone(),
                        edge_type.clone(),
                    ));
                }
            }
        }
    }

    // Step 6: Build child_to_visible mapping for aggregate edges
    // Map each symbol to its nearest visible ancestor (root or direct child of root)
    let visible_ids: HashSet<String> = symbol_map.keys().cloned().collect();
    let mut child_to_visible: HashMap<String, String> = HashMap::new();

    for symbol_id in all_symbols_map.keys() {
        // If already visible, map to itself
        if visible_ids.contains(symbol_id) {
            child_to_visible.insert(symbol_id.clone(), symbol_id.clone());
            continue;
        }

        // Walk up parent chain to find nearest visible ancestor
        let mut current = symbol_id.clone();
        let mut visited: HashSet<String> = HashSet::new();

        while let Some(parent) = parent_map.get(&current) {
            if visited.contains(parent) {
                break; // Cycle detected
            }
            visited.insert(current.clone());

            if visible_ids.contains(parent) {
                // Found nearest visible ancestor
                child_to_visible.insert(symbol_id.clone(), parent.clone());
                break;
            }

            current = parent.clone();
        }

        // If no visible ancestor found, map to root (walk up to find root)
        if !child_to_visible.contains_key(symbol_id) {
            current = symbol_id.clone();
            visited.clear();
            while let Some(parent) = parent_map.get(&current) {
                if visited.contains(parent) {
                    break;
                }
                visited.insert(current.clone());
                current = parent.clone();
            }
            // current is now the root
            child_to_visible.insert(symbol_id.clone(), current);
        }
    }

    // Step 7: Fetch ALL non-containment edges
    let all_edges = fetch_non_containment_edges(graph, &mut failed_queries).await;

    // Step 8: Compute aggregate edges using child_to_visible mapping
    let aggregate_edges = compute_aggregate_edges(&child_to_visible, &all_edges);

    build_response_with_containment(
        symbol_map,
        aggregate_edges,
        containment_edge_list,
        failed_queries,
    )
}

async fn build_subtree_data(
    graph: &CodeGraph,
    root_id: &str,
    depth: u32,
    visible_ids: Option<Vec<String>>,
) -> Result<GraphResponse, String> {
    let mut symbol_map: HashMap<String, serde_json::Value> = HashMap::new();
    let mut failed_queries: Vec<String> = Vec::new();

    // Fetch the root symbol directly by ID (avoid full table scan)
    let mut root_params = HashMap::new();
    root_params.insert(
        "symbol_id".to_string(),
        serde_json::Value::String(root_id.to_string()),
    );

    let root_symbols = match graph.execute_query("symbol_by_id", root_params).await {
        Ok(rows) => rows,
        Err(e) => {
            return Err(format!(
                "Failed to query symbol_by_id for root '{}': {}",
                root_id, e
            ));
        }
    };

    // Insert the root symbol if found
    if let Some(root_row) = root_symbols.into_iter().next() {
        symbol_map.insert(root_id.to_string(), root_row);
    } else {
        return Err(format!("Root symbol '{}' not found", root_id));
    }

    // Fetch ALL containment edges upfront (has_member, has_method, has_field)
    let (_, child_counts, containment_edges, containment_failures) =
        fetch_containment_data(graph).await;
    failed_queries.extend(containment_failures);

    // Fetch ALL symbols upfront
    let all_symbols = match graph.execute_query("all_symbols", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("all_symbols query failed: {}", e);
            failed_queries.push("all_symbols".to_string());
            Vec::new()
        }
    };

    // Build a lookup map: symbol_id -> symbol_data
    let mut symbol_lookup: HashMap<String, serde_json::Value> = HashMap::new();
    for row in all_symbols {
        if let Some(symbol_id) = row["symbol_id"].as_str() {
            symbol_lookup.insert(symbol_id.to_string(), row);
        }
    }

    // Iterative BFS to expand children to specified depth
    let mut current_level: Vec<String> = vec![root_id.to_string()];
    let mut containment_edge_list: Vec<(String, String, String)> = Vec::new();

    for _level in 0..depth {
        if current_level.is_empty() {
            break;
        }

        let mut next_level: Vec<String> = Vec::new();

        for parent_id in &current_level {
            // Get children from our containment_edges map
            if let Some(children) = containment_edges.get(parent_id) {
                for (child_id, edge_type) in children {
                    // Only add if not already in map (avoid duplicates)
                    if !symbol_map.contains_key(child_id)
                        && let Some(child_data) = symbol_lookup.get(child_id)
                    {
                        let mut child_data = child_data.clone();
                        // Inject child_count and parent_id
                        if let Some(obj) = child_data.as_object_mut() {
                            let count = child_counts.get(child_id).copied().unwrap_or(0);
                            obj.insert("child_count".to_string(), serde_json::Value::from(count));
                            obj.insert(
                                "parent_id".to_string(),
                                serde_json::Value::String(parent_id.clone()),
                            );
                        }
                        symbol_map.insert(child_id.clone(), child_data);
                        next_level.push(child_id.clone());
                    }

                    // Add containment edge to list (if child is in symbol_map)
                    if symbol_map.contains_key(child_id) {
                        containment_edge_list.push((
                            parent_id.clone(),
                            child_id.clone(),
                            edge_type.clone(),
                        ));
                    }
                }
            }
        }

        current_level = next_level;
    }

    // Get all visible symbol IDs (subtree + optional external visible_ids)
    let subtree_ids: HashSet<String> = symbol_map.keys().cloned().collect();
    let visible_set = if let Some(visible) = visible_ids {
        subtree_ids
            .iter()
            .chain(visible.iter())
            .cloned()
            .collect::<HashSet<_>>()
    } else {
        subtree_ids.clone()
    };

    // Fetch ALL non-containment edges and filter in Rust
    let all_edges = fetch_non_containment_edges(graph, &mut failed_queries).await;

    // Filter edges to only those where both endpoints are in visible set
    let mut filtered_edges: Vec<(String, String, String)> = all_edges
        .into_iter()
        .filter(|(src, dst, _)| visible_set.contains(src) && visible_set.contains(dst))
        .collect();

    // Add containment edges to the response
    filtered_edges.extend(containment_edge_list);

    build_response(symbol_map, filtered_edges, failed_queries)
}

/// Build search results: find matching symbols and return them with their ancestor chains.
/// Search is case-insensitive and matches against name or qualified_name (module_path::name).
/// Returns up to 50 matches.
async fn build_search_results(
    graph: &CodeGraph,
    search_term: &str,
) -> Result<GraphResponse, String> {
    let mut symbol_map: HashMap<String, serde_json::Value> = HashMap::new();
    let mut failed_queries: Vec<String> = Vec::new();

    // Fetch ALL symbols
    let all_symbols = match graph.execute_query("all_symbols", HashMap::new()).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("all_symbols query failed: {}", e);
            failed_queries.push("all_symbols".to_string());
            return Ok(GraphResponse {
                stats: GraphStats {
                    total_symbols: 0,
                    total_edges: 0,
                    failed_queries,
                },
                nodes: Vec::new(),
                edges: Vec::new(),
            });
        }
    };

    // Build symbol lookup
    let mut all_symbols_map: HashMap<String, serde_json::Value> = HashMap::new();
    for row in all_symbols {
        let symbol_id = row["symbol_id"].as_str().unwrap_or("").to_string();
        if symbol_id.is_empty() {
            continue;
        }
        all_symbols_map.insert(symbol_id, row);
    }

    // Search for matches (case-insensitive)
    let search_lower = search_term.to_lowercase();
    let mut matched_ids = Vec::new();

    for (symbol_id, symbol_value) in &all_symbols_map {
        let name = symbol_value["name"].as_str().unwrap_or("");
        let module_path = symbol_value["module_path"].as_str().unwrap_or("");

        let qualified_name = if module_path.is_empty() {
            name.to_string()
        } else {
            format!("{}::{}", module_path, name)
        };

        if name.to_lowercase().contains(&search_lower)
            || qualified_name.to_lowercase().contains(&search_lower)
        {
            matched_ids.push(symbol_id.clone());
            if matched_ids.len() >= 50 {
                break; // Limit to 50 matches
            }
        }
    }

    if matched_ids.is_empty() {
        return Ok(GraphResponse {
            stats: GraphStats {
                total_symbols: 0,
                total_edges: 0,
                failed_queries,
            },
            nodes: Vec::new(),
            edges: Vec::new(),
        });
    }

    // Fetch containment edges to build parent map
    let (_, child_counts, containment_edges, containment_failures) =
        fetch_containment_data(graph).await;
    failed_queries.extend(containment_failures);

    // Build parent map
    let mut parent_map: HashMap<String, String> = HashMap::new();
    for (parent_id, children) in &containment_edges {
        for (child_id, _edge_type) in children {
            parent_map.insert(child_id.clone(), parent_id.clone());
        }
    }

    // Collect matched symbols + their ancestors
    let mut nodes_to_include = HashSet::new();
    let mut containment_edge_list: Vec<(String, String, String)> = Vec::new();

    for matched_id in &matched_ids {
        // Walk up the parent chain to collect all ancestors
        let mut current = matched_id.clone();
        let mut visited = HashSet::new();

        while !current.is_empty() {
            if visited.contains(&current) {
                break; // Prevent infinite loops
            }
            visited.insert(current.clone());
            nodes_to_include.insert(current.clone());

            // Add containment edge from parent to current (if parent exists)
            if let Some(parent) = parent_map.get(&current) {
                // Find the edge type
                if let Some(children) = containment_edges.get(parent)
                    && let Some((_, edge_type)) =
                        children.iter().find(|(child, _)| child == &current)
                {
                    containment_edge_list.push((
                        parent.clone(),
                        current.clone(),
                        edge_type.clone(),
                    ));
                }
                current = parent.clone();
            } else {
                break; // Reached root
            }
        }
    }

    // Build symbol_map with child_count and parent_id
    for node_id in &nodes_to_include {
        if let Some(symbol_value) = all_symbols_map.get(node_id) {
            let mut symbol_value = symbol_value.clone();
            if let Some(obj) = symbol_value.as_object_mut() {
                let count = child_counts.get(node_id).copied().unwrap_or(0);
                obj.insert("child_count".to_string(), serde_json::Value::from(count));

                // Set parent_id
                if let Some(parent) = parent_map.get(node_id) {
                    obj.insert(
                        "parent_id".to_string(),
                        serde_json::Value::String(parent.clone()),
                    );
                } else {
                    obj.insert("parent_id".to_string(), serde_json::Value::Null);
                }
            }
            symbol_map.insert(node_id.clone(), symbol_value);
        }
    }

    build_response(symbol_map, containment_edge_list, failed_queries)
}

/// Build GraphNode list from symbol data.
/// Shared helper for both regular and aggregate responses.
fn build_nodes(symbol_map: HashMap<String, serde_json::Value>) -> Vec<GraphNode> {
    symbol_map
        .into_values()
        .filter_map(|s| {
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
                child_count: s["child_count"].as_u64().unwrap_or(0) as u32,
                parent_id: s["parent_id"].as_str().map(|s| s.to_string()),
            })
        })
        .collect()
}

fn build_response(
    symbol_map: HashMap<String, serde_json::Value>,
    all_edges: Vec<(String, String, String)>,
    failed_queries: Vec<String>,
) -> Result<GraphResponse, String> {
    let total_symbols = symbol_map.len();
    let total_edges = all_edges.len();

    let nodes = build_nodes(symbol_map);

    let edges: Vec<GraphEdge> = all_edges
        .into_iter()
        .enumerate()
        .map(|(i, (source, target, edge_type))| GraphEdge {
            id: format!("e{}", i),
            source,
            target,
            edge_type,
            aggregate_counts: None,
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

/// Build response with aggregate edges and containment edges (used for root graph view with depth-2 expansion)
fn build_response_with_containment(
    symbol_map: HashMap<String, serde_json::Value>,
    aggregate_edges: Vec<(String, String, HashMap<String, u32>)>,
    containment_edges: Vec<(String, String, String)>,
    failed_queries: Vec<String>,
) -> Result<GraphResponse, String> {
    let total_symbols = symbol_map.len();

    let nodes = build_nodes(symbol_map);

    // Convert aggregate edges
    let mut edges: Vec<GraphEdge> = aggregate_edges
        .into_iter()
        .enumerate()
        .map(|(i, (source, target, counts))| GraphEdge {
            id: format!("agg:{}", i),
            source,
            target,
            edge_type: "aggregate".to_string(),
            aggregate_counts: Some(counts),
        })
        .collect();

    // Add containment edges as regular edges
    edges.extend(containment_edges.into_iter().enumerate().map(
        |(i, (source, target, edge_type))| GraphEdge {
            id: format!("c:{}", i),
            source,
            target,
            edge_type,
            aggregate_counts: None,
        },
    ));

    let total_edges = edges.len();

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
