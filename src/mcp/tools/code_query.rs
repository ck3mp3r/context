//! Code query tool implementations
//!
//! Provides MCP tools for querying the code graph using NanoGraph.
//! Follows SOLID principles - thin MCP layer delegating to NanoGraph CLI.

use crate::common::command::format_command_error;
use crate::sync::get_data_dir;
use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content},
    schemars,
    schemars::JsonSchema,
    tool, tool_router,
};
use sanitize_filename::sanitize;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::NamedTempFile;
use tracing::{debug, error, info, warn};

// ============================================================================
// Internal types for query execution
// ============================================================================

/// Extract query name from query definition
/// Expects format: "query name(...) { ... }"
fn extract_query_name(definition: &str) -> Option<String> {
    let trimmed = definition.trim();
    if !trimmed.starts_with("query ") {
        return None;
    }

    let after_query = &trimmed[6..]; // Skip "query "
    let name_end = after_query.find('(').or_else(|| after_query.find('{'))?;
    let name = after_query[..name_end].trim();

    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

struct PreparedQuery {
    source: QuerySource,
    name: String,
    query_type: &'static str,
}

enum QuerySource {
    Temporary {
        _file: NamedTempFile, // Kept alive for auto-cleanup
        path: PathBuf,
    },
    Saved {
        path: PathBuf,
    },
}

impl QuerySource {
    fn path(&self) -> &PathBuf {
        match self {
            Self::Temporary { path, .. } => path,
            Self::Saved { path } => path,
        }
    }
}

// ============================================================================
// Parameter types
// ============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DescribeSchemaParams {
    #[schemars(description = "Repository ID from c5t database")]
    pub repo_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryCodeGraphParams {
    #[schemars(description = "Repository ID")]
    pub repo_id: String,

    #[schemars(description = "Query name to execute or save (optional)")]
    pub query_name: Option<String>,

    #[schemars(
        description = "Query definition with @description/@instruction annotations (optional)"
    )]
    pub query_definition: Option<String>,

    #[schemars(description = "Query parameters as JSON object (optional)")]
    pub params: Option<serde_json::Value>,
}

// ============================================================================
// NanoGraph CLI abstraction (for testing)
// ============================================================================

/// Trait for NanoGraph CLI operations (mockable for tests)
#[cfg_attr(test, mockall::automock)]
pub trait NanographCli: Send + Sync {
    fn get_analysis_path(&self, repo_id: &str) -> PathBuf;
    fn describe(&self, db_path: &Path) -> Result<Output, std::io::Error>;

    fn run_query(
        &self,
        db_path: &Path,
        query_file: &Path,
        query_name: &str,
        params: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Output, std::io::Error>;

    fn check_query(&self, db_path: &Path, query_file: &Path) -> Result<Output, std::io::Error>;
}

// Generate mock for testing
#[cfg(test)]
mockall::mock! {
    pub CliStub {}

    impl NanographCli for CliStub {
        fn get_analysis_path(&self, repo_id: &str) -> PathBuf;

        fn describe(&self, db_path: &Path) -> Result<Output, std::io::Error>;

        fn run_query(
            &self,
            db_path: &Path,
            query_file: &Path,
            query_name: &str,
            params: &serde_json::Map<String, serde_json::Value>,
        ) -> Result<Output, std::io::Error>;

        fn check_query(&self, db_path: &Path, query_file: &Path) -> Result<Output, std::io::Error>;
    }
}

/// Real implementation using std::process::Command
pub struct Nanograph;

impl NanographCli for Nanograph {
    fn get_analysis_path(&self, repo_id: &str) -> PathBuf {
        get_data_dir().join("repos").join(repo_id)
    }

    fn describe(&self, db_path: &Path) -> Result<Output, std::io::Error> {
        Command::new("nanograph")
            .arg("describe")
            .arg("--db")
            .arg(db_path)
            .arg("--json")
            .output()
    }

    fn run_query(
        &self,
        db_path: &Path,
        query_file: &Path,
        query_name: &str,
        params: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Output, std::io::Error> {
        let mut cmd = Command::new("nanograph");
        cmd.arg("run")
            .arg("--db")
            .arg(db_path)
            .arg("--query")
            .arg(query_file)
            .arg("--name")
            .arg(query_name)
            .arg("--format")
            .arg("json");

        // Add parameters
        for (key, value) in params {
            cmd.arg("--param");
            let param_str = if value.is_string() {
                format!("{}={}", key, value.as_str().unwrap())
            } else {
                format!("{}={}", key, value)
            };
            cmd.arg(param_str);
        }

        cmd.output()
    }

    fn check_query(&self, db_path: &Path, query_file: &Path) -> Result<Output, std::io::Error> {
        Command::new("nanograph")
            .arg("check")
            .arg("--db")
            .arg(db_path)
            .arg("--query")
            .arg(query_file)
            .output()
    }
}

// ============================================================================
// Code query tools
// ============================================================================

/// Code query tools
///
/// # SOLID Principles
/// - **Single Responsibility**: MCP interface only
/// - **Dependency Inversion**: Depends on NanographCli trait
pub struct CodeQueryTools<C: NanographCli> {
    cli: C,
    tool_router: ToolRouter<Self>,
}

// Default constructor for production use
impl CodeQueryTools<Nanograph> {
    pub fn new() -> Self {
        Self {
            cli: Nanograph,
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for CodeQueryTools<Nanograph> {
    fn default() -> Self {
        Self::new()
    }
}

// Test constructor with mock CLI
#[cfg(test)]
impl<C: NanographCli + 'static> CodeQueryTools<C> {
    pub fn new_with_cli(cli: C) -> Self {
        Self {
            cli,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl<C: NanographCli + Send + Sync + 'static> CodeQueryTools<C> {
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    /// Get schema information for a repository's code graph
    #[tool(description = "Get schema for a repository's code graph (nodes, edges, properties)")]
    pub async fn describe_schema(
        &self,
        params: Parameters<DescribeSchemaParams>,
    ) -> Result<CallToolResult, McpError> {
        let db_path = self.cli.get_analysis_path(&params.0.repo_id);
        let analysis_path = db_path.join("analysis.nano");

        // Check if analysis exists
        if !analysis_path.exists() {
            return Err(McpError::invalid_params(
                "analysis_not_found",
                Some(json!({
                    "message": format!("No analysis found for repository {}. Run c5t_code_analyze first.", params.0.repo_id),
                    "repo_id": params.0.repo_id
                })),
            ));
        }

        // Call nanograph describe
        let output = self.cli.describe(&analysis_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                McpError::internal_error(
                    "nanograph_not_found",
                    Some(json!({
                        "message": "NanoGraph CLI not found. Install with: brew install nanograph/tap/nanograph"
                    })),
                )
            } else {
                McpError::internal_error(
                    "nanograph_error",
                    Some(json!({
                        "message": e.to_string()
                    })),
                )
            }
        })?;

        // Check if command succeeded
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(McpError::internal_error(
                "nanograph_failed",
                Some(json!({
                    "message": format!("nanograph describe failed: {}", stderr)
                })),
            ));
        }

        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let schema: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| {
            McpError::internal_error(
                "json_parse_error",
                Some(json!({
                    "message": format!("Failed to parse schema JSON: {}", e)
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

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Execute queries (temporary or saved) against the code graph
    #[tool(description = "Query the code graph with temporary or saved queries")]
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

        let db_path = self.cli.get_analysis_path(&params.0.repo_id);
        let analysis_path = db_path.join("analysis.nano");

        debug!("Analysis path: {:?}", analysis_path);

        // Check if analysis exists
        if !analysis_path.exists() {
            warn!("Analysis not found at {:?}", analysis_path);
            return Err(McpError::invalid_params(
                "analysis_not_found",
                Some(json!({
                    "message": format!("No analysis found for repository {}. Run c5t_code_analyze first.", params.0.repo_id),
                    "repo_id": params.0.repo_id
                })),
            ));
        }

        // Extract params as map
        let query_params = match params.0.params {
            Some(serde_json::Value::Object(map)) => map,
            Some(_) => {
                return Err(McpError::invalid_params(
                    "invalid_params",
                    Some(json!({
                        "message": "params must be a JSON object"
                    })),
                ));
            }
            None => serde_json::Map::new(),
        };

        // Determine behavior based on what's provided
        let prepared = match (&params.0.query_name, &params.0.query_definition) {
            (None, Some(definition)) => {
                // Temp query: use NamedTempFile (automatically cleaned up on drop)
                // Extract query name from definition
                let query_name = extract_query_name(definition).ok_or_else(|| {
                    McpError::invalid_params(
                        "invalid_query",
                        Some(json!({
                            "message": "Could not extract query name from definition. Expected format: 'query name(...) { ... }'"
                        })),
                    )
                })?;

                let mut temp_file = NamedTempFile::new().map_err(|e| {
                    McpError::internal_error(
                        "fs_error",
                        Some(json!({
                            "message": format!("Failed to create temp file: {}", e)
                        })),
                    )
                })?;

                temp_file.write_all(definition.as_bytes()).map_err(|e| {
                    McpError::internal_error(
                        "fs_error",
                        Some(json!({
                            "message": format!("Failed to write temp query file: {}", e)
                        })),
                    )
                })?;

                temp_file.flush().map_err(|e| {
                    McpError::internal_error(
                        "fs_error",
                        Some(json!({
                            "message": format!("Failed to flush temp file: {}", e)
                        })),
                    )
                })?;

                let path = temp_file.path().to_path_buf();
                debug!("Created temp query file with name: {}", query_name);
                PreparedQuery {
                    source: QuerySource::Temporary {
                        _file: temp_file,
                        path,
                    },
                    name: query_name,
                    query_type: "temporary",
                }
            }
            (Some(name), None) => {
                // Execute existing saved query from queries/{sanitized_name}.gq
                let sanitized = sanitize(name);
                let query_file = analysis_path
                    .join("queries")
                    .join(format!("{}.gq", sanitized));

                if !query_file.exists() {
                    return Err(McpError::invalid_params(
                        "query_not_found",
                        Some(json!({
                            "message": format!("Saved query '{}' not found at {}", name, query_file.display()),
                            "query_name": name
                        })),
                    ));
                }

                // Extract query name from file to use with --name
                let file_content = std::fs::read_to_string(&query_file).map_err(|e| {
                    McpError::internal_error(
                        "fs_error",
                        Some(json!({
                            "message": format!("Failed to read query file: {}", e)
                        })),
                    )
                })?;

                let query_name = extract_query_name(&file_content).ok_or_else(|| {
                    McpError::internal_error(
                        "invalid_query_file",
                        Some(json!({
                            "message": format!("Could not extract query name from {}", query_file.display())
                        })),
                    )
                })?;

                PreparedQuery {
                    source: QuerySource::Saved { path: query_file },
                    name: query_name,
                    query_type: "saved",
                }
            }
            (Some(name), Some(definition)) => {
                // Save query to queries/{sanitized_name}.gq THEN execute
                let queries_dir = analysis_path.join("queries");
                std::fs::create_dir_all(&queries_dir).map_err(|e| {
                    McpError::internal_error(
                        "fs_error",
                        Some(json!({
                            "message": format!("Failed to create queries directory: {}", e)
                        })),
                    )
                })?;

                let sanitized = sanitize(name);
                let query_file = queries_dir.join(format!("{}.gq", sanitized));

                debug!("Saving query '{}' to {}", name, query_file.display());

                // Write query definition to file (overwrites if exists)
                std::fs::write(&query_file, definition).map_err(|e| {
                    McpError::internal_error(
                        "fs_error",
                        Some(json!({
                            "message": format!("Failed to write query file: {}", e)
                        })),
                    )
                })?;

                // Validate query syntax
                let check_output =
                    self.cli
                        .check_query(&analysis_path, &query_file)
                        .map_err(|e| {
                            McpError::internal_error(
                                "nanograph_error",
                                Some(json!({
                                    "message": e.to_string()
                                })),
                            )
                        })?;

                if !check_output.status.success() {
                    let error_msg = format_command_error("nanograph check", &check_output);
                    // Put the full error in the message parameter so it's visible
                    return Err(McpError::invalid_params(
                        error_msg, None, // No additional data needed
                    ));
                }

                // Extract query name from definition for --name parameter
                let query_name = extract_query_name(definition).ok_or_else(|| {
                    McpError::invalid_params(
                        "invalid_query",
                        Some(json!({
                            "message": "Could not extract query name from definition. Expected format: 'query name(...) { ... }'"
                        })),
                    )
                })?;

                PreparedQuery {
                    source: QuerySource::Saved { path: query_file },
                    name: query_name,
                    query_type: "saved_and_executed",
                }
            }
            (None, None) => unreachable!("Already validated above"),
        };

        // Execute query (temp file will be auto-cleaned on drop)
        debug!(
            "Executing query: db={:?}, query_file={:?}, name={}",
            analysis_path,
            prepared.source.path(),
            prepared.name
        );

        let output = self.cli.run_query(&analysis_path, prepared.source.path(), &prepared.name, &query_params)
            .map_err(|e| {
                error!("Nanograph command failed: {}", e);
                if e.kind() == std::io::ErrorKind::NotFound {
                    McpError::internal_error(
                        "nanograph_not_found",
                        Some(json!({
                            "message": "NanoGraph CLI not found. Install with: brew install nanograph/tap/nanograph"
                        })),
                    )
                } else {
                    McpError::internal_error(
                        "nanograph_error",
                        Some(json!({
                            "message": e.to_string()
                        })),
                    )
                }
            })?;

        // Check if command succeeded
        if !output.status.success() {
            let error_msg = format_command_error("nanograph run", &output);

            error!("{}", error_msg);

            return Err(McpError::internal_error(
                error_msg, // Put full error in message parameter
                None,
            ));
        }

        debug!("Query executed successfully");

        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let results: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| {
            error!("Failed to parse JSON: {}. Raw output: {}", e, stdout);
            McpError::internal_error(
                "json_parse_error",
                Some(json!({
                    "message": format!("Failed to parse query results: {}", e)
                })),
            )
        })?;

        info!(
            "Query completed successfully, returning {} results",
            results
                .get("rows")
                .and_then(|r| r.as_array())
                .map(|a| a.len())
                .unwrap_or(0)
        );

        let response = json!({
            "query_type": prepared.query_type,
            "query_name": prepared.name,
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

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }
}

// ============================================================================
// Helper functions
// ============================================================================
