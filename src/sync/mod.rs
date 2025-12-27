//! Sync module - Git-based synchronization for c5t data.
//!
//! This module provides functionality to export the c5t database to JSONL files
//! and sync them via Git to enable multi-machine synchronization.

mod git;
mod paths;

pub use paths::{get_data_dir, get_sync_dir};
