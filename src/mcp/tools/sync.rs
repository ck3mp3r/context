//! Sync tool - MCP interface for git-based synchronization.

use crate::db::Database;
use crate::mcp::tools::map_db_error;
use crate::sync::{RealGit, SyncError, SyncManager};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    schemars::JsonSchema,
    tool,
    tool_router,
    ErrorData as McpError,
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
}

/// Sync tools for git-based synchronization.
#[derive(Clone)]
pub struct SyncTools<D: Database> {
    db: Arc<D>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> SyncTools<D> {
    /// Create new SyncTools with database.
    pub fn new(db: Arc<D>) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    /// Get the tool router for this handler.
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    /// Sync tool - git-based synchronization.
    #[tool(description = "Git-based sync: init, export, import, or status")]
    pub async fn sync(&self, params: Parameters<SyncParams>) -> Result<CallToolResult, McpError> {
        let params = params.0;
        let manager = SyncManager::new(RealGit::new());

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
                let summary = manager
                    .export(&*self.db, params.message)
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
                let summary = manager
                    .import(&*self.db)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SqliteDatabase;

    async fn setup_test_db() -> SqliteDatabase {
        let db = SqliteDatabase::in_memory().await.unwrap();
        db.migrate().unwrap();
        db
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sync_status_not_initialized() {
        let db = Arc::new(setup_test_db().await);
        let tools = SyncTools::new(db);

        let params = SyncParams {
            operation: SyncOperation::Status,
            remote_url: None,
            message: None,
        };

        let result = tools.sync(Parameters(params)).await.unwrap();

        // Extract text from content using correct accessor
        let text = match &result.content[0].raw {
            RawContent::Text(text_content) => text_content.text.as_str(),
            _ => {
                panic!("Expected text content in test");
            }
        };

        let json: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(json["initialized"], false);
    }
}
