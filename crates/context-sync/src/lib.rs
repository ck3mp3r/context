//! Sync module - Git-based synchronization for c5t data.
//!
//! This module provides functionality to export the c5t database to JSONL files
//! and sync them via Git to enable multi-machine synchronization.

mod export;
#[cfg(test)]
mod export_test;
mod git;
#[cfg(test)]
mod git_test;
mod import;
#[cfg(test)]
mod import_test;
mod manager;
#[cfg(test)]
mod manager_test;

pub use export::{ExportError, export_all};
#[cfg(feature = "testing")]
pub use git::MockGitOps;
pub use git::{GitError, GitOps, RealGit};
pub use import::{ImportError, import_all};
pub use manager::{EntityCounts, GitStatus, InitResult, SyncError, SyncManager, SyncStatus};

// Re-export summary types from context-core for backward compatibility
pub use context_core::{ExportSummary, ImportSummary};
