//! Repository tool implementations
//!
//! Handles all MCP tools for repository management operations.
//! Follows Single Responsibility Principle (SRP).

use crate::api::notifier::{ChangeNotifier, UpdateMessage};
use crate::db::{Database, PageSort, Repo, RepoQuery, RepoRepository};
use crate::mcp::tools::{apply_limit, map_db_error};
use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    schemars::JsonSchema,
    tool, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

// Parameter types for tools
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListReposParams {
    #[schemars(
        description = "FTS5 search query (optional). Searches across remote URL, path, and tags."
    )]
    pub query: Option<String>,
    #[schemars(description = "Filter by project ID")]
    pub project_id: Option<String>,
    #[schemars(
        description = "Maximum number of repos to return (default: 10, max: 20). IMPORTANT: Keep small to prevent context overflow."
    )]
    pub limit: Option<usize>,
    #[schemars(description = "Number of items to skip (for pagination)")]
    pub offset: Option<usize>,
    #[schemars(description = "Field to sort by (remote, path, created_at). Default: created_at")]
    pub sort: Option<String>,
    #[schemars(description = "Sort order (asc, desc). Default: asc")]
    pub order: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetRepoParams {
    #[schemars(description = "Repository ID (8-character hex)")]
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateRepoParams {
    #[schemars(description = "Git remote URL")]
    pub remote: String,
    #[schemars(description = "Local file system path")]
    pub path: Option<String>,
    #[schemars(description = "Tags for categorization")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Project IDs to link (optional)")]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateRepoParams {
    #[schemars(description = "Repository ID")]
    pub id: String,
    #[schemars(description = "New remote URL")]
    pub remote: Option<String>,
    #[schemars(description = "New path")]
    pub path: Option<String>,
    #[schemars(description = "New tags")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Project IDs to link (optional)")]
    pub project_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteRepoParams {
    #[schemars(description = "Repository ID to delete")]
    pub id: String,
}

/// Repository management tools
///
/// Generic over `D: Database` for zero-cost abstraction.
///
/// # SOLID Principles
/// - **Single Responsibility**: Only handles repository operations
/// - **Dependency Inversion**: Depends on Database trait
#[derive(Clone)]
pub struct RepoTools<D: Database> {
    db: Arc<D>,
    notifier: ChangeNotifier,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> RepoTools<D> {
    /// Create new RepoTools with database
    pub fn new(db: Arc<D>, notifier: ChangeNotifier) -> Self {
        Self {
            db,
            notifier,
            tool_router: Self::tool_router(),
        }
    }

    /// Get the tool router for this handler
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    /// List repositories with pagination and sorting (default: 10, max: 20)
    #[tool(
        description = "List repositories with pagination and sorting. Sort by remote, path, or created_at. Default limit: 10, max: 20 to prevent context overflow."
    )]
    pub async fn list_repos(
        &self,
        params: Parameters<ListReposParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = apply_limit(params.0.limit);

        let query = RepoQuery {
            page: PageSort {
                limit: Some(limit),
                offset: params.0.offset,
                sort_by: params.0.sort.clone(),
                sort_order: match params.0.order.as_deref() {
                    Some("desc") => Some(crate::db::SortOrder::Desc),
                    Some("asc") => Some(crate::db::SortOrder::Asc),
                    _ => None,
                },
            },
            tags: None,
            project_id: params.0.project_id,
            search_query: params.0.query,
        };

        let result = self
            .db
            .repos()
            .list(Some(&query))
            .await
            .map_err(map_db_error)?;

        let response = json!({
            "items": result.items,
            "total": result.total,
            "limit": result.limit,
            "offset": result.offset,
        });

        let content = serde_json::to_string_pretty(&response).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get a repository by ID
    #[tool(description = "Get a repository by ID")]
    pub async fn get_repo(
        &self,
        params: Parameters<GetRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        let repo = self
            .db
            .repos()
            .get(&params.0.id)
            .await
            .map_err(map_db_error)?;

        let content = serde_json::to_string_pretty(&repo).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Create a new repository
    #[tool(description = "Create a new repository")]
    pub async fn create_repo(
        &self,
        params: Parameters<CreateRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        let repo = Repo {
            id: String::new(), // Repository generates this
            remote: params.0.remote,
            path: params.0.path,
            tags: params.0.tags.unwrap_or_default(),
            project_ids: params.0.project_ids.unwrap_or_default(),
            created_at: String::new(), // Repository generates this
        };

        let created = self.db.repos().create(&repo).await.map_err(map_db_error)?;

        self.notifier.notify(UpdateMessage::RepoCreated {
            repo_id: created.id.clone(),
        });

        let content = serde_json::to_string_pretty(&created).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Update a repository
    #[tool(description = "Update a repository")]
    pub async fn update_repo(
        &self,
        params: Parameters<UpdateRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        // Get existing repo
        let mut repo = self
            .db
            .repos()
            .get(&params.0.id)
            .await
            .map_err(map_db_error)?;

        // Update fields if provided
        if let Some(r) = params.0.remote {
            repo.remote = r;
        }
        if let Some(p) = params.0.path {
            repo.path = Some(p);
        }
        if let Some(tags) = params.0.tags {
            repo.tags = tags;
        }
        if let Some(project_ids) = params.0.project_ids {
            repo.project_ids = project_ids;
        }

        self.db.repos().update(&repo).await.map_err(map_db_error)?;

        // Get the updated repo to return it
        let updated = self
            .db
            .repos()
            .get(&params.0.id)
            .await
            .map_err(map_db_error)?;

        self.notifier.notify(UpdateMessage::RepoUpdated {
            repo_id: params.0.id.clone(),
        });

        let content = serde_json::to_string_pretty(&updated).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Delete a repository
    #[tool(description = "Delete a repository")]
    pub async fn delete_repo(
        &self,
        params: Parameters<DeleteRepoParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .repos()
            .delete(&params.0.id)
            .await
            .map_err(map_db_error)?;

        self.notifier.notify(UpdateMessage::RepoDeleted {
            repo_id: params.0.id.clone(),
        });

        let content = serde_json::json!({
            "success": true,
            "message": format!("Repository {} deleted successfully", params.0.id)
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&content).unwrap(),
        )]))
    }
}
