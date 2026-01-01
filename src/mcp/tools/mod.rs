//! MCP tool implementations
//!
//! This module contains tool handlers organized by entity type.
//! Each module follows Single Responsibility Principle (SRP).

// Pagination limits to prevent context overflow
/// Default limit for list operations (when not specified by user)
pub(crate) const DEFAULT_LIMIT: usize = 10;

/// Maximum limit for list operations (hard cap to prevent context overflow)
/// CRITICAL: Keep this small! Large responses break the agent's context window.
pub(crate) const MAX_LIMIT: usize = 20;

/// Apply limit with default and max cap
///
/// Returns the capped limit value, ensuring it's between DEFAULT_LIMIT and MAX_LIMIT.
/// If user_limit is None, returns DEFAULT_LIMIT.
/// If user_limit exceeds MAX_LIMIT, returns MAX_LIMIT.
pub(crate) fn apply_limit(user_limit: Option<usize>) -> usize {
    match user_limit {
        Some(limit) => limit.min(MAX_LIMIT),
        None => DEFAULT_LIMIT,
    }
}

pub mod notes;
#[cfg(test)]
mod notes_test;
pub mod projects;
#[cfg(test)]
mod projects_test;
pub mod repos;
#[cfg(test)]
mod repos_test;
pub mod sync;
#[cfg(test)]
mod sync_test;
pub mod task_lists;
#[cfg(test)]
mod task_lists_test;
pub mod tasks;
#[cfg(test)]
mod tasks_test;

pub use notes::NoteTools;
pub use projects::ProjectTools;
pub use repos::RepoTools;
pub use sync::SyncTools;
pub use task_lists::TaskListTools;
pub use tasks::TaskTools;

use crate::db::DbError;
use rmcp::ErrorData as McpError;

/// Convert DbError to McpError with appropriate error codes and messages
pub(crate) fn map_db_error(err: DbError) -> McpError {
    match err {
        DbError::NotFound { entity_type, id } => McpError::invalid_params(
            "not_found",
            Some(serde_json::json!({
                "entity_type": entity_type,
                "id": id,
                "message": format!("{} with id '{}' not found", entity_type, id)
            })),
        ),
        DbError::AlreadyExists { entity_type, id } => McpError::invalid_params(
            "already_exists",
            Some(serde_json::json!({
                "entity_type": entity_type,
                "id": id,
                "message": format!("{} with id '{}' already exists", entity_type, id)
            })),
        ),
        DbError::InvalidData { message, help } => McpError::invalid_params(
            "invalid_data",
            Some(serde_json::json!({
                "message": message,
                "help": help
            })),
        ),
        DbError::Validation { message } => McpError::invalid_params(
            "validation_error",
            Some(serde_json::json!({
                "message": message
            })),
        ),
        DbError::Database { message } => {
            // Parse common SQLite errors for better messages
            if message.contains("FOREIGN KEY constraint failed") {
                McpError::invalid_params(
                    "foreign_key_violation",
                    Some(serde_json::json!({
                        "message": "Referenced entity does not exist",
                        "details": message
                    })),
                )
            } else if message.contains("UNIQUE constraint failed") {
                McpError::invalid_params(
                    "unique_constraint_violation",
                    Some(serde_json::json!({
                        "message": "Duplicate value for unique field",
                        "details": message
                    })),
                )
            } else if message.contains("NOT NULL constraint failed") {
                let field = message
                    .split("NOT NULL constraint failed: ")
                    .nth(1)
                    .unwrap_or("unknown field");
                McpError::invalid_params(
                    "required_field_missing",
                    Some(serde_json::json!({
                        "message": format!("Required field '{}' is missing", field),
                        "field": field
                    })),
                )
            } else if message.contains("CHECK constraint failed") {
                McpError::invalid_params(
                    "validation_failed",
                    Some(serde_json::json!({
                        "message": "Data validation failed",
                        "details": message
                    })),
                )
            } else {
                McpError::internal_error(
                    "database_error",
                    Some(serde_json::json!({
                        "message": message
                    })),
                )
            }
        }
        DbError::Migration { message } => McpError::internal_error(
            "migration_error",
            Some(serde_json::json!({
                "message": message
            })),
        ),
        DbError::Connection { message } => McpError::internal_error(
            "connection_error",
            Some(serde_json::json!({
                "message": message
            })),
        ),
        DbError::Constraint { message } => McpError::invalid_params(
            "constraint_violation",
            Some(serde_json::json!({
                "message": message
            })),
        ),
    }
}
