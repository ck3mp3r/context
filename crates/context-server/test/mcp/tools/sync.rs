//! Co-located tests for the MCP sync tool status output.
//!
//! The MCP status output must expose the same count keys as the REST status
//! endpoint: repos, projects, task_lists, tasks, notes, skills, attachments,
//! and total. Each object carries distinct values so a mix-up between the
//! database, sync_files, and tombstones objects fails the assertions.

use context_sync::{EntityCounts, GitStatus, SyncStatus};
use rmcp::model::ErrorCode;

use super::{counts_object, status_json};

const KEYS: [&str; 8] = [
    "repos",
    "projects",
    "task_lists",
    "tasks",
    "notes",
    "skills",
    "attachments",
    "total",
];

fn counts(base: usize) -> EntityCounts {
    EntityCounts {
        repos: base,
        projects: base + 1,
        task_lists: base + 2,
        tasks: base + 3,
        notes: base + 4,
        skills: base + 5,
        attachments: base + 6,
    }
}

fn initialized_status() -> SyncStatus {
    SyncStatus {
        initialized: true,
        remote_url: Some("git@example.com:test/repo.git".to_string()),
        git_status: Some(GitStatus {
            clean: true,
            status_output: String::new(),
        }),
        db_counts: Some(counts(1)),
        jsonl_counts: Some(counts(21)),
        tombstone_counts: Some(counts(11)),
        last_synced_at: Some("2024-01-01 00:00:00".to_string()),
        schema_version: 5,
    }
}

fn assert_counts(object: &serde_json::Value, base: usize, label: &str) {
    for (offset, key) in KEYS.iter().enumerate() {
        let expected = if *key == "total" {
            (base..base + 7).sum::<usize>()
        } else {
            base + offset
        };
        assert_eq!(
            object.get(*key).and_then(serde_json::Value::as_u64),
            Some(expected as u64),
            "{label} must carry {key} = {expected}, got: {object}"
        );
    }
}

#[test]
fn counts_object_includes_all_eight_keys() {
    let object = counts_object(&counts(1));

    assert_eq!(
        object.as_object().map(serde_json::Map::len),
        Some(KEYS.len()),
        "count object must carry exactly the 8 REST keys, got: {object}"
    );
    assert_counts(&object, 1, "counts_object");
}

#[test]
fn status_json_database_and_sync_files_include_all_eight_keys() {
    let status = initialized_status();

    let value = status_json(&status).expect("initialized status should build JSON");

    assert_eq!(value["initialized"], true);
    assert_eq!(value["remote_url"], "git@example.com:test/repo.git");
    assert_eq!(value["last_synced_at"], "2024-01-01 00:00:00");
    assert_eq!(value["schema_version"], 5);
    assert_eq!(value["git"]["clean"], true);
    assert_eq!(value["git"]["status"], "No changes");

    assert_counts(&value["database"], 1, "database");
    assert_counts(&value["sync_files"], 21, "sync_files");
}

#[test]
fn status_json_keeps_tombstone_counts() {
    let status = initialized_status();

    let value = status_json(&status).expect("initialized status should build JSON");

    assert_counts(&value["tombstones"], 11, "tombstones");
}

#[test]
fn status_json_omits_count_objects_when_counts_are_absent() {
    let mut status = initialized_status();
    status.jsonl_counts = None;
    status.tombstone_counts = None;

    let value = status_json(&status).expect("initialized status should build JSON");

    assert_eq!(value["sync_files"], serde_json::Value::Null);
    assert_eq!(value["tombstones"], serde_json::Value::Null);
    assert_counts(&value["database"], 1, "database");
}

#[test]
fn status_json_returns_invalid_status_when_db_counts_missing() {
    let mut status = initialized_status();
    status.db_counts = None;

    let err = status_json(&status).expect_err("missing db_counts must be an error");

    assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
    assert_eq!(err.message, "invalid_status");
    assert_eq!(
        err.data
            .as_ref()
            .and_then(|data| data.get("error"))
            .and_then(serde_json::Value::as_str),
        Some("Status structure inconsistent (initialized but missing data)")
    );
}

#[test]
fn status_json_returns_invalid_status_when_git_status_missing() {
    let mut status = initialized_status();
    status.git_status = None;

    let err = status_json(&status).expect_err("missing git_status must be an error");

    assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
    assert_eq!(err.message, "invalid_status");
}
