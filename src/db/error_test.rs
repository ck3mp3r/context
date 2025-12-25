//! Tests for database error types.

use crate::db::{DbError, DbResult};

#[test]
fn not_found_error_displays_correctly() {
    let err = DbError::NotFound {
        entity_type: "project".to_string(),
        id: "abc12345".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "Entity not found: project with id 'abc12345'"
    );
}

#[test]
fn already_exists_error_displays_correctly() {
    let err = DbError::AlreadyExists {
        entity_type: "repo".to_string(),
        id: "xyz78901".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "Entity already exists: repo with id 'xyz78901'"
    );
}

#[test]
fn invalid_data_error_displays_correctly() {
    let err = DbError::InvalidData {
        message: "title cannot be empty".to_string(),
        help: "Provide a non-empty title".to_string(),
    };
    assert_eq!(err.to_string(), "Invalid data: title cannot be empty");
}

#[test]
fn database_error_displays_correctly() {
    let err = DbError::Database {
        message: "constraint violation".to_string(),
    };
    assert_eq!(err.to_string(), "Database error: constraint violation");
}

#[test]
fn migration_error_displays_correctly() {
    let err = DbError::Migration {
        message: "failed to apply migration 0002".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "Migration error: failed to apply migration 0002"
    );
}

#[test]
fn connection_error_displays_correctly() {
    let err = DbError::Connection {
        message: "unable to open database".to_string(),
    };
    assert_eq!(err.to_string(), "Connection error: unable to open database");
}

#[test]
fn db_result_ok_returns_value() {
    let result: DbResult<i32> = Ok(42);
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn db_result_err_returns_error() {
    let result: DbResult<i32> = Err(DbError::NotFound {
        entity_type: "task".to_string(),
        id: "12345678".to_string(),
    });
    assert!(result.is_err());
}
