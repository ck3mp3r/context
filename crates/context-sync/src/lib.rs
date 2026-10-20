//! Sync module - Git-based synchronization for c5t data.
//!
//! This module provides functionality to export the c5t database to JSONL files
//! and sync them via Git to enable multi-machine synchronization.

mod git;
mod manager;

#[cfg(test)]
#[path = "../test/git.rs"]
mod git_test;
#[cfg(test)]
#[path = "../test/manager.rs"]
mod manager_test;

#[cfg(feature = "testing")]
pub use git::MockGitOps;
pub use git::{GitError, GitOps, RealGit};
pub use manager::{EntityCounts, GitStatus, InitResult, SyncError, SyncManager, SyncStatus};

// Re-export summary types from context-core for backward compatibility
pub use context_core::{ExportSummary, ImportSummary};
