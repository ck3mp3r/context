//! Sync module - Git-based synchronization for c5t data.
//!
//! This module provides functionality to export the c5t database to JSONL files
//! and sync them via Git to enable multi-machine synchronization.

mod export;
mod git;
mod import;
mod jsonl;
mod manager;
mod paths;

pub use export::{ExportError, ExportSummary, export_all};
pub use git::{GitError, GitOps, RealGit};
pub use import::{ImportError, ImportSummary, import_all};
pub use jsonl::{JsonlError, read_jsonl, write_jsonl};
pub use manager::{EntityCounts, GitStatus, SyncError, SyncManager, SyncStatus};
pub use paths::{get_data_dir, get_db_path, get_sync_dir};
