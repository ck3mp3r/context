//! Database abstraction layer with SOLID principles.
//!
//! This module provides trait-based abstractions for data access,
//! allowing different storage backends (SQLite, PostgreSQL, in-memory, etc.)
//! to be swapped without changing business logic.
//!
//! # Architecture
//!
//! - `error`: Storage-agnostic error types
//! - `models`: Domain entities (Project, Repo, TaskList, Task, Note)
//! - `repository`: Trait definitions for data access

mod error;
mod models;
mod repository;
pub mod sqlite;

pub use error::{DbError, DbResult};
pub use models::*;
pub use repository::*;
pub use sqlite::SqliteDatabase;
