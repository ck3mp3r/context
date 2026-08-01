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
//! - `utils`: Database utility functions
//!
//! # Note
//!
//! The core types (models, error, repository, utils) have been moved to
//! the `context-core` crate. This module re-exports them for backward
//! compatibility. This shim will be removed in Phase 3 when db/sqlite
//! moves to context-db.

pub mod sqlite;

// Re-export everything from context-core for backward compatibility
// so existing `crate::db::*` imports continue to work.
pub use context_core::*;
pub use sqlite::SqliteDatabase;
