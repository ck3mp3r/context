//! Code analysis tool implementations
//!
//! Handles MCP tools for code analysis operations.
//! Follows SOLID principles - thin MCP layer delegating to service layer.

use crate::a6s;
use crate::a6s::store::surrealdb;
use crate::a6s::tracker::{AnalysisStatus, AnalysisTracker};
use crate::a6s::types::{GraphStats, PipelineProgress};
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
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

// Parameter types
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeCodeParams {
    #[schemars(description = "Repository ID from c5t database")]
    pub repo_id: String,

    #[schemars(
        description = "Action to perform: 'analyze' (start analysis, default) or 'status' (check current status)"
    )]
    pub action: Option<String>,
}

/// Code analysis tools
///
/// # SOLID Principles
/// - **Single Responsibility**: MCP interface only
/// - **Dependency Inversion**: Depends on Database trait and service layer
#[derive(Clone)]
pub struct CodeAnalysisTools<D: Database> {
    db: Arc<D>,
    analysis_db: Arc<surrealdb::SurrealDbConnection>,
    tracker: AnalysisTracker,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> CodeAnalysisTools<D> {
    pub fn new(
        db: Arc<D>,
        analysis_db: Arc<surrealdb::SurrealDbConnection>,
        tracker: AnalysisTracker,
    ) -> Self {
        Self {
            db,
            analysis_db,
            tracker,
            tool_router: Self::tool_router(),
        }
    }

    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    #[tool(description = "Analyze a repository's code or check analysis status")]
    pub async fn analyze_code(
        &self,
        params: Parameters<AnalyzeCodeParams>,
    ) -> Result<CallToolResult, McpError> {
        let action = params.0.action.as_deref().unwrap_or("analyze");
        match action {
            "status" => self.check_status(&params.0.repo_id),
            _ => self.start_analysis(params).await,
        }
    }

    fn check_status(&self, repo_id: &str) -> Result<CallToolResult, McpError> {
        let response = match self.tracker.get(repo_id) {
            None => json!({
                "status": "idle",
                "message": format!("No analysis has been run for repository {}.", repo_id),
            }),
            Some(AnalysisStatus::Analyzing { phase }) => {
                let mut response = json!({
                    "status": "analyzing",
                    "message": format!("Analysis is in progress for repository {}.", repo_id),
                });
                if let Some(p) = phase {
                    response["phase"] = json!(p);
                }
                response
            }
            Some(AnalysisStatus::Complete { stats }) => json!({
                "status": "complete",
                "stats": {
                    "total_symbols": stats.total_symbols,
                    "total_edges": stats.total_edges,
                    "symbol_counts": stats.symbol_counts,
                },
            }),
            Some(AnalysisStatus::Failed { error }) => json!({
                "status": "failed",
                "error": error,
            }),
        };

        let content = serde_json::to_string_pretty(&response).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(json!({ "error": e.to_string() })),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    async fn start_analysis(
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

        // Set tracker to analyzing before spawning
        let repo_id = params.0.repo_id.clone();
        if !self.tracker.try_set_analyzing(&repo_id) {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json!({
                    "status": "already_analyzing",
                    "message": format!("Analysis is already in progress for repository {}. Check status with action='status'.", repo_id),
                    "repo_id": repo_id,
                }))
                .unwrap(),
            )]));
        }

        let repo_path = PathBuf::from(&repo_path_str);
        let analysis_db = Arc::clone(&self.analysis_db);
        let tracker = self.tracker.clone();

        tokio::spawn(async move {
            tracing::info!("Starting a6s analysis for repo: {}", repo_id);

            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<PipelineProgress>(16);
            let tracker_for_progress = tracker.clone();
            let repo_id_for_progress = repo_id.clone();

            // Relay pipeline progress to tracker phases
            tokio::spawn(async move {
                while let Some(progress) = progress_rx.recv().await {
                    let phase = match &progress {
                        PipelineProgress::Scanned(_) => "Extracting",
                        PipelineProgress::Extracted(_) => "Resolving",
                        PipelineProgress::Resolved(_) => "Loading",
                        PipelineProgress::Loaded => "Committing",
                    };
                    tracker_for_progress.set_phase(&repo_id_for_progress, phase);
                }
            });

            let commit_hash = "HEAD";

            match a6s::analyze(
                &repo_path,
                &repo_id,
                commit_hash,
                Some(progress_tx),
                analysis_db,
            )
            .await
            {
                Ok(stats) => {
                    tracing::info!(
                        "a6s analysis complete: {} symbols, {} edges resolved, {} dropped",
                        stats.symbols_registered,
                        stats.edges_resolved,
                        stats.edges_dropped
                    );
                    tracker.set_complete(
                        &repo_id,
                        GraphStats {
                            total_symbols: stats.symbols_registered,
                            total_edges: stats.edges_resolved,
                            symbol_counts: HashMap::new(),
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("a6s analysis failed: {:?}", e);
                    tracker.set_failed(&repo_id, e.to_string());
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
