//! Database error types.
//!
//! This module provides abstracted error types for database operations.
//! It uses miette for fancy diagnostic output and thiserror for derive macros.
//! The error types are storage-backend agnostic.

use miette::Diagnostic;
use thiserror::Error;

/// Database operation errors.
#[derive(Error, Diagnostic, Debug)]
pub enum DbError {
    #[error("Entity not found: {entity_type} with id '{id}'")]
    #[diagnostic(code(context::db::not_found))]
    NotFound { entity_type: String, id: String },

    #[error("Entity already exists: {entity_type} with id '{id}'")]
    #[diagnostic(code(context::db::already_exists))]
    AlreadyExists { entity_type: String, id: String },

    #[error("Invalid data: {message}")]
    #[diagnostic(code(context::db::invalid_data), help("{help}"))]
    InvalidData { message: String, help: String },

    #[error("Database error: {message}")]
    #[diagnostic(code(context::db::database_error))]
    Database { message: String },

    #[error("Migration error: {message}")]
    #[diagnostic(
        code(context::db::migration_error),
        help("Check your migration files and database state")
    )]
    Migration { message: String },

    #[error("Connection error: {message}")]
    #[diagnostic(
        code(context::db::connection_error),
        help("Verify the database path and permissions")
    )]
    Connection { message: String },
}

/// Result type for database operations.
pub type DbResult<T> = Result<T, DbError>;
