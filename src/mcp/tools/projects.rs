//! Project tool implementations
//!
//! Handles all MCP tools for project management operations.
//! Follows Single Responsibility Principle (SRP).

use crate::db::{Database, PageSort, Project, ProjectQuery, ProjectRepository};
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
use std::sync::Arc;

// Parameter types for tools
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListProjectsParams {
    #[schemars(
        description = "Maximum number of projects to return (default: 10, max: 20). IMPORTANT: Keep this small to prevent context overflow."
    )]
    pub limit: Option<usize>,
    #[schemars(
        description = "Field to sort by (title, created_at, updated_at). Default: created_at"
    )]
    pub sort: Option<String>,
    #[schemars(description = "Sort order (asc, desc). Default: asc")]
    pub order: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetProjectParams {
    #[schemars(description = "Project ID (8-character hex)")]
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateProjectParams {
    #[schemars(description = "Project title")]
    pub title: String,
    #[schemars(description = "Optional description")]
    pub description: Option<String>,
    #[schemars(description = "Tags for categorization")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateProjectParams {
    #[schemars(description = "Project ID")]
    pub id: String,
    #[schemars(description = "New title")]
    pub title: Option<String>,
    #[schemars(description = "New description")]
    pub description: Option<String>,
    #[schemars(description = "New tags")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteProjectParams {
    #[schemars(description = "Project ID to delete")]
    pub id: String,
}

/// Project management tools
///
/// Generic over `D: Database` for zero-cost abstraction.
///
/// # SOLID Principles
/// - **Single Responsibility**: Only handles project operations
/// - **Dependency Inversion**: Depends on Database trait
#[derive(Clone)]
pub struct ProjectTools<D: Database> {
    db: Arc<D>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<D: Database + 'static> ProjectTools<D> {
    /// Create new ProjectTools with database
    pub fn new(db: Arc<D>) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    /// Get the tool router for this handler
    pub fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    /// List projects with pagination and sorting (default: 10, max: 20)
    #[tool(
        description = "List projects with pagination and sorting. Sort by title, created_at, or updated_at. Default limit: 10, max: 20 to prevent context overflow."
    )]
    pub async fn list_projects(
        &self,
        params: Parameters<ListProjectsParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = apply_limit(params.0.limit);

        let query = ProjectQuery {
            page: PageSort {
                limit: Some(limit),
                offset: None,
                sort_by: params.0.sort.clone(),
                sort_order: match params.0.order.as_deref() {
                    Some("desc") => Some(crate::db::SortOrder::Desc),
                    Some("asc") => Some(crate::db::SortOrder::Asc),
                    _ => None,
                },
            },
            tags: None,
        };

        let result = self
            .db
            .projects()
            .list(Some(&query))
            .await
            .map_err(map_db_error)?;

        let content = serde_json::to_string_pretty(&result.items).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Get a project by ID
    #[tool(description = "Get a project by ID")]
    pub async fn get_project(
        &self,
        params: Parameters<GetProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        let project = self
            .db
            .projects()
            .get(&params.0.id)
            .await
            .map_err(map_db_error)?;

        let content = serde_json::to_string_pretty(&project).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Create a new project
    #[tool(description = "Create a new project")]
    pub async fn create_project(
        &self,
        params: Parameters<CreateProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        let project = Project {
            id: String::new(), // Repository generates this
            title: params.0.title,
            description: params.0.description,
            tags: params.0.tags.unwrap_or_default(),
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(), // Repository generates this
            updated_at: String::new(), // Repository generates this
        };

        let created = self
            .db
            .projects()
            .create(&project)
            .await
            .map_err(map_db_error)?;

        let content = serde_json::to_string_pretty(&created).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Update a project
    #[tool(description = "Update a project")]
    pub async fn update_project(
        &self,
        params: Parameters<UpdateProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        // Get existing project
        let mut project = self
            .db
            .projects()
            .get(&params.0.id)
            .await
            .map_err(map_db_error)?;

        // Update fields if provided
        if let Some(t) = params.0.title {
            project.title = t;
        }
        if let Some(d) = params.0.description {
            project.description = Some(d);
        }
        if let Some(tags) = params.0.tags {
            project.tags = tags;
        }

        self.db
            .projects()
            .update(&project)
            .await
            .map_err(map_db_error)?;

        // Get the updated project to return it
        let updated = self
            .db
            .projects()
            .get(&params.0.id)
            .await
            .map_err(map_db_error)?;

        let content = serde_json::to_string_pretty(&updated).map_err(|e| {
            McpError::internal_error(
                "serialization_error",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    /// Delete a project
    #[tool(description = "Delete a project")]
    pub async fn delete_project(
        &self,
        params: Parameters<DeleteProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.db
            .projects()
            .delete(&params.0.id)
            .await
            .map_err(map_db_error)?;

        let content = serde_json::json!({
            "success": true,
            "message": format!("Project {} deleted successfully", params.0.id)
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&content).unwrap(),
        )]))
    }
}
