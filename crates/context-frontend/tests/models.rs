//! Pins the frontend `Note` deserialization contract for the archive fields.
//!
//! The wasm crate cannot host-link its own test harness, so the model module is included by path
//! and compiled into this binary: no wasm, no browser, and no rendering.

// The model module is included whole, so the parts this binary does not pin are dead here.
#![allow(dead_code)]

#[path = "../src/models/mod.rs"]
mod models;

use models::Note;

fn base_note_json() -> serde_json::Value {
    serde_json::json!({
        "id": "0123abcd",
        "title": "A note",
        "content": "body",
        "tags": [],
        "project_ids": [],
        "repo_ids": [],
        "created_at": "2026-01-01 00:00:00",
        "updated_at": "2026-01-01 00:00:00",
    })
}

#[test]
fn note_without_status_defaults_to_active() {
    let note: Note = serde_json::from_value(base_note_json()).expect("Note should deserialize");

    assert_eq!(note.status, "active");
    assert_eq!(note.archived_at, None);
}

#[test]
fn note_with_status_carries_archive_fields() {
    let mut json = base_note_json();
    json["status"] = serde_json::json!("archived");
    json["archived_at"] = serde_json::json!("2026-02-02 10:00:00");

    let note: Note = serde_json::from_value(json).expect("Note should deserialize");

    assert_eq!(note.status, "archived");
    assert_eq!(note.archived_at.as_deref(), Some("2026-02-02 10:00:00"));
}

#[test]
fn active_note_serializes_without_archived_at() {
    let note: Note = serde_json::from_value(base_note_json()).expect("Note should deserialize");

    let value = serde_json::to_value(&note).expect("Note should serialize");

    assert_eq!(value["status"], serde_json::json!("active"));
    assert!(value.get("archived_at").is_none());
}
