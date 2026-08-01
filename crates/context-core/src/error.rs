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

    #[error("Invalid data: {message} (hint: {help})")]
    #[diagnostic(code(context::db::invalid_data))]
    InvalidData { message: String, help: String },

    #[error("Validation error: {message}")]
    #[diagnostic(code(context::db::validation_error))]
    Validation { message: String },

    #[error("Database error: {message}")]
    #[diagnostic(code(context::db::database_error))]
    Database { message: String },

    #[error("Migration error: {message}")]
    #[diagnostic(code(context::db::migration_error))]
    Migration { message: String },

    #[error("Connection error: {message}")]
    #[diagnostic(code(context::db::connection_error))]
    Connection { message: String },

    #[error("Constraint violation: {message}")]
    #[diagnostic(code(context::db::constraint))]
    Constraint { message: String },
}

/// Result type for database operations.
pub type DbResult<T> = Result<T, DbError>;
