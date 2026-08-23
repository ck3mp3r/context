//! Tests for Note MCP tools
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use context_core::{HasNotes, HasProjects, Note, NoteRepository, Project, ProjectRepository};
use context_db::SqliteDatabase;
use context_server::api::notifier::ChangeNotifier;
use context_server::mcp::tools::notes::{
    CreateNoteParams, DeleteNoteParams, EditNoteParams, LineRange, ListNotesParams, NoteTools,
    ReadNoteParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ContentBlock;
use std::sync::Arc;

async fn setup_db() -> SqliteDatabase {
    common::setup_db().await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_empty() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let params = ListNotesParams {
        query: None,
        tags: None,
        project_id: None,
        parent_id: None,
        note_type: None,
        limit: None,
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools.list_notes(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_note() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let params = CreateNoteParams {
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: Some(vec!["test".to_string()]),
        project_ids: None,
        repo_ids: None,
        parent_id: None,
        idx: None,
    };

    let result = tools.create_note(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["title"], "Test Note");
    assert_eq!(response["content"], "Test content");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_note_empty_title() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let params = CreateNoteParams {
        title: "".to_string(),
        content: "Some content".to_string(),
        tags: None,
        project_ids: None,
        repo_ids: None,
        parent_id: None,
        idx: None,
    };

    let result = tools.create_note(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_note() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    let params = ReadNoteParams {
        note_id: "note0001".to_string(),
        format: None,
        ranges: None,
    };

    let result = tools.read_note(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["title"], "Test Note");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_note_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let params = ReadNoteParams {
        note_id: "nonexist".to_string(),
        format: None,
        ranges: None,
    };

    let result = tools.read_note(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_note() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "To Delete".to_string(),
        content: "Content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    let params = DeleteNoteParams {
        note_id: "note0001".to_string(),
    };

    let result = tools.delete_note(Parameters(params)).await;
    assert!(result.is_ok());

    let result = db.notes().get("note0001").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_note_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let params = DeleteNoteParams {
        note_id: "nonexist".to_string(),
    };

    let result = tools.delete_note(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_note() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "Original".to_string(),
        content: "Original content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    // First read the note to get the etag
    let read_params = ReadNoteParams {
        note_id: "note0001".to_string(),
        format: Some("json".to_string()),
        ranges: None,
    };
    let read_result = tools.read_note(Parameters(read_params)).await.unwrap();
    let read_text = match &read_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let read_response: serde_json::Value = serde_json::from_str(read_text).unwrap();
    let etag = read_response["etag"].as_str().unwrap().to_string();

    let params = EditNoteParams {
        note_id: "note0001".to_string(),
        etag,
        title: Some("Updated".to_string()),
        tags: Some(vec!["updated".to_string()]),
        project_ids: None,
        repo_ids: None,
        parent_id: None,
        idx: None,
        patches: vec![],
    };

    let result = tools.edit_note(Parameters(params)).await;
    assert!(result.is_ok());

    let updated = db.notes().get("note0001").await.unwrap();
    assert_eq!(updated.title, "Updated");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_note_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let params = EditNoteParams {
        note_id: "nonexist".to_string(),
        etag: "some-etag".to_string(),
        title: Some("Updated".to_string()),
        tags: None,
        project_ids: None,
        repo_ids: None,
        parent_id: None,
        idx: None,
        patches: vec![],
    };

    let result = tools.edit_note(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_data() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: vec!["test".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    let params = ListNotesParams {
        query: None,
        tags: None,
        project_id: None,
        parent_id: None,
        note_type: None,
        limit: None,
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools.list_notes(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_search() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "Rust Backend".to_string(),
        content: "Some content about backend".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    let params = ListNotesParams {
        query: Some("rust".to_string()),
        tags: None,
        project_id: None,
        parent_id: None,
        note_type: None,
        limit: None,
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools.list_notes(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_pagination() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    for i in 0..3 {
        let note = Note {
            id: format!("note{:04}", i),
            title: format!("Note {}", i),
            content: "Content".to_string(),
            tags: vec![],
            parent_id: None,
            idx: None,
            repo_ids: vec![],
            project_ids: vec![],
            subnote_count: None,
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        };
        db.notes().create(&note).await.unwrap();
    }

    let params = ListNotesParams {
        query: None,
        tags: None,
        project_id: None,
        parent_id: None,
        note_type: None,
        limit: Some(1),
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools.list_notes(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["items"].as_array().unwrap().len(), 1);
    assert_eq!(response["total"], 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_tag_filter() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "Tagged Note".to_string(),
        content: "Content".to_string(),
        tags: vec!["backend".to_string(), "rust".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    let params = ListNotesParams {
        query: None,
        tags: Some(vec!["backend".to_string()]),
        project_id: None,
        parent_id: None,
        note_type: None,
        limit: None,
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools.list_notes(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_project_id_filter() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Create the project first to satisfy FK constraint
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.projects().create(&project).await.unwrap();

    let note = Note {
        id: "note0001".to_string(),
        title: "Project Note".to_string(),
        content: "Content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    let params = ListNotesParams {
        query: None,
        tags: None,
        project_id: Some("proj0001".to_string()),
        parent_id: None,
        note_type: None,
        limit: None,
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools.list_notes(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_parent_id_filter() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let parent = Note {
        id: "parent01".to_string(),
        title: "Parent".to_string(),
        content: "Parent content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&parent).await.unwrap();

    let child = Note {
        id: "child001".to_string(),
        title: "Child".to_string(),
        content: "Child content".to_string(),
        tags: vec![],
        parent_id: Some("parent01".to_string()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&child).await.unwrap();

    let params = ListNotesParams {
        query: None,
        tags: None,
        project_id: None,
        parent_id: Some("parent01".to_string()),
        note_type: None,
        limit: None,
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools.list_notes(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
    assert_eq!(response["items"][0]["id"], "child001");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_note_type_filter() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let parent = Note {
        id: "parent01".to_string(),
        title: "Parent".to_string(),
        content: "Parent content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&parent).await.unwrap();

    let child = Note {
        id: "child001".to_string(),
        title: "Child".to_string(),
        content: "Child content".to_string(),
        tags: vec![],
        parent_id: Some("parent01".to_string()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&child).await.unwrap();

    let params = ListNotesParams {
        query: None,
        tags: None,
        project_id: None,
        parent_id: None,
        note_type: Some("task".to_string()),
        limit: None,
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools.list_notes(Parameters(params)).await;
    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_sorting() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    for (id, title) in [
        ("note0001", "Charlie"),
        ("note0002", "Alpha"),
        ("note0003", "Bravo"),
    ] {
        let note = Note {
            id: id.to_string(),
            title: title.to_string(),
            content: "Content".to_string(),
            tags: vec![],
            parent_id: None,
            idx: None,
            repo_ids: vec![],
            project_ids: vec![],
            subnote_count: None,
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        };
        db.notes().create(&note).await.unwrap();
    }

    let params = ListNotesParams {
        query: None,
        tags: None,
        project_id: None,
        parent_id: None,
        note_type: None,
        limit: None,
        offset: None,
        include_content: None,
        sort: Some("title".to_string()),
        order: Some("asc".to_string()),
    };

    let result = tools.list_notes(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let titles: Vec<&str> = response["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Alpha", "Bravo", "Charlie"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_toon_format() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Line 1\nLine 2\nLine 3".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    let params = ReadNoteParams {
        note_id: "note0001".to_string(),
        format: Some("toon".to_string()),
        ranges: None,
    };

    let result = tools.read_note(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    assert!(content_text.contains("lines[3]"));
    assert!(content_text.contains("Line 1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_json_format() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    let params = ReadNoteParams {
        note_id: "note0001".to_string(),
        format: Some("json".to_string()),
        ranges: None,
    };

    let result = tools.read_note(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["title"], "Test Note");
    assert_eq!(response["content"], "Test content");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_with_ranges() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    let params = ReadNoteParams {
        note_id: "note0001".to_string(),
        format: Some("toon".to_string()),
        ranges: Some(vec![LineRange { start: 2, end: 3 }]),
    };

    let result = tools.read_note(Parameters(params)).await;
    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_note_with_patches() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Line 1\nLine 2\nLine 3".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    // First read the note to get the etag
    let read_params = ReadNoteParams {
        note_id: "note0001".to_string(),
        format: Some("json".to_string()),
        ranges: None,
    };
    let read_result = tools.read_note(Parameters(read_params)).await.unwrap();
    let read_text = match &read_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let read_response: serde_json::Value = serde_json::from_str(read_text).unwrap();
    let etag = read_response["etag"].as_str().unwrap().to_string();

    let params = EditNoteParams {
        note_id: "note0001".to_string(),
        etag,
        title: None,
        tags: None,
        project_ids: None,
        repo_ids: None,
        parent_id: None,
        idx: None,
        patches: vec![],
    };

    let result = tools.edit_note(Parameters(params)).await;
    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_note_with_etag() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Original content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    // First read the note to get the etag
    let read_params = ReadNoteParams {
        note_id: "note0001".to_string(),
        format: Some("json".to_string()),
        ranges: None,
    };
    let read_result = tools.read_note(Parameters(read_params)).await.unwrap();
    let read_text = match &read_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let read_response: serde_json::Value = serde_json::from_str(read_text).unwrap();
    let etag = read_response["etag"].as_str().unwrap().to_string();

    let params = EditNoteParams {
        note_id: "note0001".to_string(),
        etag,
        title: Some("Updated".to_string()),
        tags: None,
        project_ids: None,
        repo_ids: None,
        parent_id: None,
        idx: None,
        patches: vec![],
    };

    let result = tools.edit_note(Parameters(params)).await;
    assert!(result.is_ok());

    let updated = db.notes().get("note0001").await.unwrap();
    assert_eq!(updated.title, "Updated");
}
