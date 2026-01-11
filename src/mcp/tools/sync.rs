//! Sync tool - MCP interface for git-based synchronization.
//!
//! # SOLID Principles
//!
//! This module follows the Dependency Inversion Principle (DIP):
//! - SyncTools is generic over `D: Database` and `G: GitOps`
//! - Dependencies are INJECTED via constructor, not created internally
//! - Tests can inject MockGitOps for isolated testing
//! - Production code uses with_real_git() convenience constructor

use crate::db::Database;
use crate::mcp::tools::map_db_error;
use crate::sync::{GitOps, RealGit, SyncError, SyncManager};
use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    schemars::JsonSchema,
    tool, tool_router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Sync operation types.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SyncOperation {
    /// Initialize sync (create git repo, add remote)
    Init,
    /// Export database to JSONL and push
    Export,
    /// Pull from remote and import JSONL to database
    Import,
    /// Show sync status
    Status,
}

/// Parameters for the sync tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SyncParams {
    /// Sync operation to perform
    #[schemars(description = "Operation: init, export, import, or status")]
    pub operation: SyncOperation,

    /// Git remote URL (required for init if not already configured)
    #[schemars(description = "Git remote URL (optional, for init operation)")]
    pub remote_url: Option<String>,

    /// Custom commit message (for export operation)
    #[schemars(description = "Commit message (optional, for export operation)")]
    pub message: Option<String>,

    /// Use remote for sync operations (push on export, pull on import)
    #[schemars(
        description = "Use remote: push after export or pull before import (optional, default: false)"
    )]
    pub remote: Option<bool>,
}

/// Sync tools for git-based synchronization.
///
/// # SOLID: Dependency Inversion Principle (DIP)
///
/// SyncTools is generic over both Database and GitOps:
/// - `D: Database` - Database backend (e.g., SqliteDatabase, or mock in tests)
/// - `G: GitOps` - Git operations (e.g., RealGit for production, MockGitOps for tests)
///
/// Dependencies are INJECTED via constructors:
/// - `with_manager(db, manager)` - Full control (for tests)
/// - `with_real_git(db)` - Convenience for production (uses RealGit + production paths)
/// - `new(db)` - DEPRECATED: Kept for backward compatibility, use with_real_git() instead
#[derive(Clone)]
pub struct SyncTools<D: Database, G: GitOps + Send + Sync> {
    db: Arc<D>,
    manager: SyncManager<G>,
    tool_router: ToolRouter<Self>,
}

impl<D: Database + 'static, G: GitOps + Send + Sync + 'static> SyncTools<D, G> {
    /// Create new SyncTools with injected SyncManager (RECOMMENDED for tests).
    ///
    /// This constructor follows SOLID DIP by accepting the SyncManager
    /// as an injected dependency, allowing full control over sync directory
    /// and git operations.
    ///
    /// # Example (Test)
    /// ```ignore
    /// use std::sync::Arc;
    /// use tempfile::TempDir;
    /// use context::db::SqliteDatabase;
    /// use context::sync::{MockGitOps, SyncManager};
    /// use context::mcp::tools::SyncTools;
    ///
    /// # async fn example() {
    /// let db = Arc::new(SqliteDatabase::in_memory().await.unwrap());
    /// let temp_dir = TempDir::new().unwrap();
    /// let mock_git = MockGitOps::new();
    /// let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    /// let tools = SyncTools::with_manager(db, manager);
    /// # }
    /// ```
    pub fn with_manager(db: Arc<D>, manager: SyncManager<G>) -> Self {
        Self {
            db,
            manager,
            tool_router: Self::tool_router(),
        }
    }

    /// Get the tool router for this handler.
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }
}

// Convenience constructor for production use with RealGit
impl<D: Database + 'static> SyncTools<D, RealGit> {
    /// Create new SyncTools with RealGit and production paths (RECOMMENDED for production).
    ///
    /// This is a convenience constructor that creates a SyncManager with:
    /// - RealGit (actual git commands)
    /// - Production sync directory (~/.local/share/c5t/sync/)
    ///
    /// For tests, use `with_manager()` with MockGitOps and TempDir instead.
    ///
    /// # Example (Production)
    /// ```no_run
    /// use std::sync::Arc;
    /// use context::db::SqliteDatabase;
    /// use context::mcp::tools::SyncTools;
    ///
    /// # async fn example() {
    /// let db = Arc::new(SqliteDatabase::in_memory().await.unwrap());
    /// let tools = SyncTools::with_real_git(db);
    /// # }
    /// ```
    pub fn with_real_git(db: Arc<D>) -> Self {
        let manager = SyncManager::new(RealGit::new());
        Self::with_manager(db, manager)
    }

    /// Create new SyncTools with database (DEPRECATED).
    ///
    /// # Deprecation
    /// This method is deprecated. Use `with_real_git()` for production
    /// or `with_manager()` for tests.
    ///
    /// This constructor is kept for backward compatibility but will be
    /// removed in a future version.
    #[deprecated(since = "0.1.0", note = "Use `with_real_git()` instead")]
    pub fn new(db: Arc<D>) -> Self {
        Self::with_real_git(db)
    }
}

#[tool_router]
impl<D: Database + 'static, G: GitOps + Send + Sync + 'static> SyncTools<D, G> {
    /// Sync tool - git-based synchronization.
    #[tool(description = "Git-based sync: init, export, import, or status")]
    pub async fn sync(&self, params: Parameters<SyncParams>) -> Result<CallToolResult, McpError> {
        let params = params.0;
        let manager = &self.manager;

        let content = match params.operation {
            SyncOperation::Init => {
                manager
                    .init(params.remote_url)
                    .await
                    .map_err(map_sync_error)?;

                serde_json::json!({
                    "status": "success",
                    "message": "Sync initialized successfully",
                    "sync_dir": crate::sync::get_sync_dir().display().to_string(),
                })
            }

            SyncOperation::Export => {
                let remote = params.remote.unwrap_or(false);
                let summary = manager
                    .export(&*self.db, params.message, remote)
                    .await
                    .map_err(map_sync_error)?;

                serde_json::json!({
                    "status": "success",
                    "message": "Export completed",
                    "exported": {
                        "repos": summary.repos,
                        "projects": summary.projects,
                        "task_lists": summary.task_lists,
                        "tasks": summary.tasks,
                        "notes": summary.notes,
                        "total": summary.total(),
                    }
                })
            }

            SyncOperation::Import => {
                let remote = params.remote.unwrap_or(false);
                let summary = manager
                    .import(&*self.db, remote)
                    .await
                    .map_err(map_sync_error)?;

                serde_json::json!({
                    "status": "success",
                    "message": "Import completed",
                    "imported": {
                        "repos": summary.repos,
                        "projects": summary.projects,
                        "task_lists": summary.task_lists,
                        "tasks": summary.tasks,
                        "notes": summary.notes,
                        "total": summary.total(),
                    }
                })
            }

            SyncOperation::Status => {
                let status = manager.status(&*self.db).await.map_err(map_sync_error)?;

                if !status.initialized {
                    serde_json::json!({
                        "initialized": false,
                        "message": "Sync not initialized. Run 'init' operation first.",
                    })
                } else {
                    // These should always be Some when initialized=true, but handle gracefully
                    match (status.git_status.as_ref(), status.db_counts.as_ref()) {
                        (Some(git_status), Some(db_counts)) => {
                            serde_json::json!({
                                "initialized": true,
                                "remote_url": status.remote_url,
                                "git": {
                                    "clean": git_status.clean,
                                    "status": if git_status.clean { "No changes" } else { &git_status.status_output },
                                },
                                "database": {
                                    "repos": db_counts.repos,
                                    "projects": db_counts.projects,
                                    "task_lists": db_counts.task_lists,
                                    "tasks": db_counts.tasks,
                                    "notes": db_counts.notes,
                                    "total": db_counts.total(),
                                },
                                "sync_files": status.jsonl_counts.as_ref().map(|counts| {
                                    serde_json::json!({
                                        "repos": counts.repos,
                                        "projects": counts.projects,
                                        "task_lists": counts.task_lists,
                                        "tasks": counts.tasks,
                                        "notes": counts.notes,
                                        "total": counts.total(),
                                    })
                                }),
                            })
                        }
                        _ => {
                            // This should never happen, but return safe error instead of panic
                            return Err(McpError::internal_error(
                                "invalid_status",
                                Some(serde_json::json!({
                                    "error": "Status structure inconsistent (initialized but missing data)",
                                })),
                            ));
                        }
                    }
                }
            }
        };

        let content_str = serde_json::to_string_pretty(&content).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content_str)]))
    }
}

/// Map SyncError to McpError.
fn map_sync_error(err: SyncError) -> McpError {
    match err {
        SyncError::NotInitialized => McpError::invalid_params(
            "not_initialized",
            Some(serde_json::json!({
                "error": "Sync not initialized. Run init operation first.",
            })),
        ),
        SyncError::Database(db_err) => map_db_error(db_err),
        SyncError::Git(git_err) => McpError::internal_error(
            "git_error",
            Some(serde_json::json!({
                "error": git_err.to_string(),
            })),
        ),
        SyncError::Export(export_err) => McpError::internal_error(
            "export_error",
            Some(serde_json::json!({
                "error": export_err.to_string(),
            })),
        ),
        SyncError::Import(import_err) => McpError::internal_error(
            "import_error",
            Some(serde_json::json!({
                "error": import_err.to_string(),
            })),
        ),
        SyncError::Io(io_err) => McpError::internal_error(
            "io_error",
            Some(serde_json::json!({
                "error": io_err.to_string(),
            })),
        ),
    }
}
