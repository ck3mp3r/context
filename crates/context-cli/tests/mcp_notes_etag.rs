//! Tests for the note etag contract
//!
//! Covers etag length, determinism, distinctness across updated_at values,
//! stale-etag rejection in edit_note, and the stale-etag error payload.
//! Uses the test harness in common::setup_db().

mod common;

use context_core::{HasNotes, Note, NoteRepository};
use context_db::SqliteDatabase;
use context_server::api::notifier::ChangeNotifier;
use context_server::mcp::tools::notes::{EditNoteParams, NoteTools, ReadNoteParams};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ContentBlock;
use std::sync::Arc;

async fn setup_db() -> SqliteDatabase {
    common::setup_db().await
}

async fn read_etag<D: HasNotes + 'static>(tools: &NoteTools<D>, note_id: &str) -> String {
    let params = ReadNoteParams {
        note_id: note_id.to_string(),
        format: Some("json".to_string()),
        ranges: None,
    };
    let result = tools
        .read_note(Parameters(params))
        .await
        .expect("read_note should succeed for an existing note");
    let content_text = match &result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value =
        serde_json::from_str(content_text).expect("read_note output should be valid JSON");
    response["etag"]
        .as_str()
        .expect("read_note response should contain an etag")
        .to_string()
}

fn make_note(id: &str, updated_at: &str) -> Note {
    Note {
        id: id.to_string(),
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some(updated_at.to_string()),
        deleted_at: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_etag_is_eight_characters() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    db.notes()
        .create(&make_note("note0001", "2025-01-01 00:00:00"))
        .await
        .expect("Create should succeed");

    let etag = read_etag(&tools, "note0001").await;

    assert_eq!(etag.len(), 8, "etag should be 8 characters, got: {etag}");
    assert!(
        etag.chars().all(|c| c.is_ascii_hexdigit()),
        "etag should be hex, got: {etag}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_etag_is_deterministic() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    db.notes()
        .create(&make_note("note0001", "2025-01-01 00:00:00"))
        .await
        .expect("Create should succeed");

    let first = read_etag(&tools, "note0001").await;
    let second = read_etag(&tools, "note0001").await;

    assert_eq!(
        first, "a938adc3",
        "etag should be the first 8 hex chars of SHA256(updated_at)"
    );
    assert_eq!(
        first, second,
        "etag should not change while the note is unmodified"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_etag_differs_for_different_updated_at() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    db.notes()
        .create(&make_note("note0001", "2025-01-01 00:00:00"))
        .await
        .expect("Create should succeed");
    db.notes()
        .create(&make_note("note0002", "2025-01-01 00:00:01"))
        .await
        .expect("Create should succeed");

    let first = read_etag(&tools, "note0001").await;
    let second = read_etag(&tools, "note0002").await;

    assert_ne!(
        first, second,
        "notes with different updated_at should have different etags"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_note_rejects_stale_etag() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    db.notes()
        .create(&make_note("note0001", "2025-01-01 00:00:00"))
        .await
        .expect("Create should succeed");

    let params = EditNoteParams {
        note_id: "note0001".to_string(),
        etag: "deadbeef".to_string(),
        title: Some("Updated".to_string()),
        tags: None,
        project_ids: None,
        repo_ids: None,
        parent_id: None,
        idx: None,
        patches: vec![],
    };

    let error = tools
        .edit_note(Parameters(params))
        .await
        .expect_err("edit_note should reject a stale etag");

    assert!(
        error.message.contains("re-read"),
        "error should tell the caller to re-read, got: {}",
        error.message
    );

    let unchanged = db
        .notes()
        .get("note0001")
        .await
        .expect("Get should succeed");
    assert_eq!(
        unchanged.title, "Test Note",
        "note should be unchanged after a rejected edit"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_note_error_payload_omits_expected_etag() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    db.notes()
        .create(&make_note("note0001", "2025-01-01 00:00:00"))
        .await
        .expect("Create should succeed");

    let params = EditNoteParams {
        note_id: "note0001".to_string(),
        etag: "deadbeef".to_string(),
        title: Some("Updated".to_string()),
        tags: None,
        project_ids: None,
        repo_ids: None,
        parent_id: None,
        idx: None,
        patches: vec![],
    };

    let error = tools
        .edit_note(Parameters(params))
        .await
        .expect_err("edit_note should reject a stale etag");

    assert_eq!(
        error.message.as_ref(),
        "Note has been modified since last read. Please re-read the note before editing.",
        "error message should instruct the caller to re-read"
    );

    let data = error.data.expect("error should carry a data payload");
    let object = data
        .as_object()
        .expect("error data should be a JSON object");

    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["note_id", "provided_etag"],
        "payload should expose only note_id and provided_etag"
    );
    assert_eq!(data["note_id"], "note0001");
    assert_eq!(data["provided_etag"], "deadbeef");

    let unchanged = db
        .notes()
        .get("note0001")
        .await
        .expect("Get should succeed");
    assert_eq!(
        unchanged.title, "Test Note",
        "note should be unchanged after a rejected edit"
    );
}
