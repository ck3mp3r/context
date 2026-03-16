//! Tests for Note MCP tools

use crate::api::notifier::ChangeNotifier;
use crate::db::{Database, Note, NoteRepository, SqliteDatabase};
use crate::mcp::tools::notes::{
    CreateNoteParams, DeleteNoteParams, EditNoteParams, ListNotesParams, NoteTools, ReadNoteParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::RawContent;
use sha2::Digest;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_empty() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
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

    let result = tools
        .list_notes(Parameters(params))
        .await
        .expect("list_notes should succeed");

    // Parse JSON response
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    // Empty database should have 0 notes
    assert_eq!(json["total"], 0);
    assert_eq!(json["items"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_and_read_note() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Create note
    let create_params = CreateNoteParams {
        title: "Meeting Notes".to_string(),
        content: "# Meeting with team\n\n- Discussed project timeline\n- Assigned tasks"
            .to_string(),
        tags: Some(vec!["meeting".to_string(), "team".to_string()]),
        parent_id: None,
        idx: None,
        repo_ids: None,
        project_ids: None,
    };

    let result = tools
        .create_note(Parameters(create_params))
        .await
        .expect("create should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let created: Note = serde_json::from_str(content_text).unwrap();

    assert_eq!(created.title, "Meeting Notes");
    assert!(created.content.contains("Meeting with team"));
    assert_eq!(
        created.tags,
        vec!["meeting".to_string(), "team".to_string()]
    );
    assert!(!created.id.is_empty());

    // Read the note (full content - ranges omitted)
    let read_params = ReadNoteParams {
        note_id: created.id.clone(),
        ranges: None, // Omit ranges for full content
        format: None, // Default JSON format
    };

    let result = tools
        .read_note(Parameters(read_params))
        .await
        .expect("read should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let retrieved: Note = serde_json::from_value(json.clone()).unwrap();

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.title, "Meeting Notes");

    // Verify etag field exists and is valid
    assert!(json.get("etag").is_some(), "etag field should exist");
    let etag1 = json["etag"].as_str().expect("etag should be a string");
    assert!(!etag1.is_empty(), "etag should not be empty");
    assert_eq!(etag1.len(), 64, "etag should be 64 chars (SHA256 hex)");

    // Read again - same note should return same etag (idempotent)
    let read_params2 = ReadNoteParams {
        note_id: created.id.clone(),
        ranges: None,
        format: None,
    };

    let result2 = tools
        .read_note(Parameters(read_params2))
        .await
        .expect("second read should succeed");

    let content_text2 = match &result2.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json2: serde_json::Value = serde_json::from_str(content_text2).unwrap();
    let etag2 = json2["etag"].as_str().expect("etag should be a string");

    assert_eq!(etag1, etag2, "same note read twice should return same etag");

    // Verify etag is deterministic - same updated_at should produce same etag
    let updated_at = json["updated_at"]
        .as_str()
        .expect("updated_at should exist");
    let expected_etag = {
        let mut hasher = sha2::Sha256::new();
        sha2::Digest::update(&mut hasher, updated_at.as_bytes());
        format!("{:x}", hasher.finalize())
    };
    assert_eq!(etag1, expected_etag, "etag should match SHA256(updated_at)");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_not_found() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let params = ReadNoteParams {
        note_id: "nonexist".to_string(),
        ranges: None,
        format: None,
    };

    let result = tools.read_note(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_tag_filter() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create notes with different tags
    let note1 = Note {
        id: String::new(),
        title: "Work Note".to_string(),
        content: "Content 1".to_string(),
        tags: vec!["work".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let note2 = Note {
        id: String::new(),
        title: "Personal Note".to_string(),
        content: "Content 2".to_string(),
        tags: vec!["personal".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // List only "work" notes
    let params = ListNotesParams {
        query: None,
        tags: Some(vec!["work".to_string()]),
        project_id: None,
        parent_id: None,
        note_type: None,
        limit: None,
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_notes(Parameters(params))
        .await
        .expect("list should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["title"], "Work Note");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_note() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a note
    let note = Note {
        id: String::new(),
        title: "Original Title".to_string(),
        content: "Original content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created = db.notes().create(&note).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Read note to get initial etag
    let read_params = ReadNoteParams {
        note_id: created.id.clone(),
        ranges: None,
        format: None,
    };
    let read_result = tools
        .read_note(Parameters(read_params))
        .await
        .expect("read should succeed");
    let read_text = match &read_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let read_json: serde_json::Value = serde_json::from_str(read_text).unwrap();
    let etag = read_json["etag"].as_str().expect("etag should exist");

    // Update note metadata only (no patches)
    let edit_params = EditNoteParams {
        note_id: created.id.clone(),
        etag: etag.to_string(),
        title: Some("Updated Title".to_string()),
        tags: Some(vec!["updated".to_string()]),
        parent_id: None,
        idx: None,
        repo_ids: None,
        project_ids: None,
        patches: vec![], // Empty patches for metadata-only update
    };

    let result = tools
        .edit_note(Parameters(edit_params))
        .await
        .expect("edit should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let updated: Note = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.title, "Updated Title");
    assert_eq!(updated.content, "Original content"); // Content unchanged
    assert_eq!(updated.tags, vec!["updated".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_note() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a note
    let note = Note {
        id: String::new(),
        title: "To be deleted".to_string(),
        content: "This will be removed".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created = db.notes().create(&note).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Delete note
    let delete_params = DeleteNoteParams {
        note_id: created.id.clone(),
    };

    let result = tools
        .delete_note(Parameters(delete_params))
        .await
        .expect("delete should succeed");

    // Verify success message
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    assert!(content_text.contains("deleted"));

    // Verify note is gone
    let get_result = db.notes().get(&created.id).await;
    assert!(get_result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_notes() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create notes with searchable content
    let note1 = Note {
        id: String::new(),
        title: "Rust Programming".to_string(),
        content: "Learning about Rust ownership and borrowing".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let note2 = Note {
        id: String::new(),
        title: "Python Tutorial".to_string(),
        content: "Python list comprehensions and generators".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Search for "Rust"
    let params = ListNotesParams {
        query: Some("Rust".to_string()),
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

    let result = tools
        .list_notes(Parameters(params))
        .await
        .expect("search should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["title"], "Rust Programming");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_notes_with_tag_filter() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create notes
    let note1 = Note {
        id: String::new(),
        title: "Rust Async".to_string(),
        content: "Async programming in Rust".to_string(),
        tags: vec!["rust".to_string(), "async".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let note2 = Note {
        id: String::new(),
        title: "Rust Basics".to_string(),
        content: "Basic Rust syntax and types".to_string(),
        tags: vec!["rust".to_string(), "basics".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Search for "Rust" with "async" tag filter
    let params = ListNotesParams {
        query: Some("Rust".to_string()),
        tags: Some(vec!["async".to_string()]),
        project_id: None,
        parent_id: None,
        note_type: None,
        limit: None,
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_notes(Parameters(params))
        .await
        .expect("search should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["title"], "Rust Async");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_sort_and_order() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create notes with specific timestamps for sorting
    let note1 = Note {
        id: String::new(),
        title: "First Note".to_string(),
        content: "First content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 10:00:00".to_string()),
        updated_at: Some("2025-01-01 10:00:00".to_string()),
    };

    let note2 = Note {
        id: String::new(),
        title: "Second Note".to_string(),
        content: "Second content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-02 10:00:00".to_string()),
        updated_at: Some("2025-01-03 10:00:00".to_string()),
    };

    let note3 = Note {
        id: String::new(),
        title: "Third Note".to_string(),
        content: "Third content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-03 10:00:00".to_string()),
        updated_at: Some("2025-01-02 10:00:00".to_string()),
    };

    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();
    db.notes().create(&note3).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Test sorting by updated_at DESC
    let params = ListNotesParams {
        query: None,
        tags: None,
        project_id: None,
        parent_id: None,
        note_type: None,
        limit: None,
        offset: None,
        include_content: Some(false),
        sort: Some("updated_at".to_string()),
        order: Some("desc".to_string()),
    };

    let result = tools
        .list_notes(Parameters(params))
        .await
        .expect("list_notes should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 3);
    let items = json["items"].as_array().unwrap();
    // Should be ordered by updated_at DESC: note2 (2025-01-03), note3 (2025-01-02), note1 (2025-01-01)
    assert_eq!(items[0]["title"], "Second Note");
    assert_eq!(items[1]["title"], "Third Note");
    assert_eq!(items[2]["title"], "First Note");

    // Test sorting by title ASC
    let params = ListNotesParams {
        query: None,
        tags: None,
        project_id: None,
        parent_id: None,
        note_type: None,
        limit: None,
        offset: None,
        include_content: Some(false),
        sort: Some("title".to_string()),
        order: Some("asc".to_string()),
    };

    let result = tools
        .list_notes(Parameters(params))
        .await
        .expect("list_notes should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 3);
    let items = json["items"].as_array().unwrap();
    // Should be ordered by title ASC
    assert_eq!(items[0]["title"], "First Note");
    assert_eq!(items[1]["title"], "Second Note");
    assert_eq!(items[2]["title"], "Third Note");
}

// =============================================================================
// Hierarchical Notes Tests (parent_id and idx)
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_note_with_parent() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Create parent note
    let parent_params = CreateNoteParams {
        title: "Parent Note".to_string(),
        content: "Parent content".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        repo_ids: None,
        project_ids: None,
    };

    let parent_result = tools
        .create_note(Parameters(parent_params))
        .await
        .expect("create parent should succeed");

    let parent_text = match &parent_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let parent: Note = serde_json::from_str(parent_text).unwrap();

    // Create child note with parent_id
    let child_params = CreateNoteParams {
        title: "Child Note".to_string(),
        content: "Child content".to_string(),
        tags: None,
        parent_id: Some(parent.id.clone()),
        idx: None,
        repo_ids: None,
        project_ids: None,
    };

    let child_result = tools
        .create_note(Parameters(child_params))
        .await
        .expect("create child should succeed");

    let child_text = match &child_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let child: Note = serde_json::from_str(child_text).unwrap();

    assert_eq!(child.title, "Child Note");
    assert_eq!(child.parent_id, Some(parent.id));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_note_idx() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Create note with idx
    let create_params = CreateNoteParams {
        title: "Test Note".to_string(),
        content: "Content".to_string(),
        tags: None,
        parent_id: None,
        idx: Some(10),
        repo_ids: None,
        project_ids: None,
    };

    let create_result = tools
        .create_note(Parameters(create_params))
        .await
        .expect("create should succeed");

    let create_text = match &create_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let created: Note = serde_json::from_str(create_text).unwrap();
    assert_eq!(created.idx, Some(10));

    // Read note to get etag
    let read_params = ReadNoteParams {
        note_id: created.id.clone(),
        ranges: None,
        format: None,
    };
    let read_result = tools
        .read_note(Parameters(read_params))
        .await
        .expect("read should succeed");
    let read_text = match &read_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let read_json: serde_json::Value = serde_json::from_str(read_text).unwrap();
    let etag = read_json["etag"].as_str().expect("etag should exist");

    // Update idx using edit_note
    let edit_params = EditNoteParams {
        note_id: created.id.clone(),
        etag: etag.to_string(),
        title: Some("Test Note".to_string()),
        tags: None,
        parent_id: None,
        idx: Some(Some(20)),
        repo_ids: None,
        project_ids: None,
        patches: vec![], // Empty patches for metadata-only update
    };

    let update_result = tools
        .edit_note(Parameters(edit_params))
        .await
        .expect("edit should succeed");

    let update_text = match &update_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let updated: Note = serde_json::from_str(update_text).unwrap();
    assert_eq!(updated.idx, Some(20));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_subnotes() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Create parent note
    let parent_params = CreateNoteParams {
        title: "Parent".to_string(),
        content: "Parent content".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        repo_ids: None,
        project_ids: None,
    };

    let parent_result = tools
        .create_note(Parameters(parent_params))
        .await
        .expect("create parent should succeed");

    let parent_text = match &parent_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let parent: Note = serde_json::from_str(parent_text).unwrap();

    // Create child notes with different idx values
    let children = vec![("Child 1", 30), ("Child 2", 10), ("Child 3", 20)];

    for (title, idx) in children {
        let child_params = CreateNoteParams {
            title: title.to_string(),
            content: "Content".to_string(),
            tags: None,
            parent_id: Some(parent.id.clone()),
            idx: Some(idx),
            repo_ids: None,
            project_ids: None,
        };

        tools
            .create_note(Parameters(child_params))
            .await
            .expect("create child should succeed");
    }

    // List subnotes filtered by parent_id
    let list_params = ListNotesParams {
        query: None,
        tags: None,
        project_id: None,
        parent_id: Some(parent.id.clone()),
        note_type: None,
        limit: None,
        offset: None,
        include_content: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_notes(Parameters(list_params))
        .await
        .expect("list should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(json["total"], 3);
    // Should be ordered by idx (10, 20, 30)
    assert_eq!(items[0]["title"], "Child 2");
    assert_eq!(items[0]["idx"], 10);
    assert_eq!(items[1]["title"], "Child 3");
    assert_eq!(items[1]["idx"], 20);
    assert_eq!(items[2]["title"], "Child 1");
    assert_eq!(items[2]["idx"], 30);
}

#[test]
fn test_line_range_schema_is_object_with_start_end() {
    use rmcp::handler::server::tool::schema_for_type;
    use rmcp::handler::server::wrapper::Parameters;
    let schema = schema_for_type::<Parameters<ReadNoteParams>>();
    let json = serde_json::Value::Object((*schema).clone());

    let ranges_prop = &json["properties"]["ranges"];
    // Must be array type (possibly nullable: type: ["array","null"])
    let is_array_type = ranges_prop["type"] == "array"
        || ranges_prop["type"]
            .as_array()
            .map(|a| a.iter().any(|v| v == "array"))
            .unwrap_or(false);
    assert!(is_array_type, "ranges must be array type");
    // items must be inlined object, not a $ref
    let items = &ranges_prop["items"];
    assert!(
        items.get("$ref").is_none(),
        "ranges items must not use $ref (causes GPT-4.1 failure)"
    );
    assert_eq!(
        items["type"], "object",
        "ranges items must be inlined object"
    );
    assert!(
        items["properties"]["start"].is_object(),
        "LineRange must have start"
    );
    assert!(
        items["properties"]["end"].is_object(),
        "LineRange must have end"
    );
}

#[test]
fn test_line_patch_schema_is_object_with_start_end_content() {
    use rmcp::handler::server::tool::schema_for_type;
    use rmcp::handler::server::wrapper::Parameters;
    let schema = schema_for_type::<Parameters<EditNoteParams>>();
    let json = serde_json::Value::Object((*schema).clone());

    let patches = &json["properties"]["patches"];
    assert_eq!(patches["type"], "array");
    // items must be inlined object, not a $ref
    let items = &patches["items"];
    assert!(
        items.get("$ref").is_none(),
        "patches items must not use $ref (causes GPT-4.1 failure)"
    );
    assert_eq!(
        items["type"], "object",
        "patches items must be inlined object"
    );
    assert!(
        items["properties"]["start"].is_object(),
        "LinePatch must have start"
    );
    assert!(
        items["properties"]["end"].is_object(),
        "LinePatch must have end"
    );
    assert!(
        items["properties"]["content"].is_object(),
        "LinePatch must have content"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_toon_format() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Create a note with multi-line content including commas
    let create_params = CreateNoteParams {
        title: "Test Note".to_string(),
        content: "First line\nSecond line, with comma\nThird line".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        repo_ids: None,
        project_ids: None,
    };

    let result = tools
        .create_note(Parameters(create_params))
        .await
        .expect("create should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let created: Note = serde_json::from_str(content_text).unwrap();

    // Read with TOON format
    let read_params = ReadNoteParams {
        note_id: created.id.clone(),
        ranges: None,
        format: Some("toon".to_string()),
    };

    let result = tools
        .read_note(Parameters(read_params))
        .await
        .expect("read should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    // Parse JSON response to extract TOON content
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let toon_content = json["content"].as_str().expect("content should be string");

    // Verify TOON format structure
    assert!(toon_content.contains("lines[3]{ln,text}:"));
    assert!(toon_content.contains("1,First line"));
    assert!(
        toon_content.contains("\"2,Second line, with comma\"")
            || toon_content.contains("2,\"Second line, with comma\"")
    );
    assert!(toon_content.contains("3,Third line"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_toon_format_with_ranges() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Create a note with 5 lines
    let create_params = CreateNoteParams {
        title: "Multi-line Note".to_string(),
        content: "Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        repo_ids: None,
        project_ids: None,
    };

    let result = tools
        .create_note(Parameters(create_params))
        .await
        .expect("create should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let created: Note = serde_json::from_str(content_text).unwrap();

    // Read lines 2-4 with TOON format
    let read_params = ReadNoteParams {
        note_id: created.id.clone(),
        ranges: Some(vec![crate::mcp::tools::notes::LineRange {
            start: 2,
            end: 4,
        }]),
        format: Some("toon".to_string()),
    };

    let result = tools
        .read_note(Parameters(read_params))
        .await
        .expect("read should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    // Parse JSON response
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let toon_content = json["content"].as_str().expect("content should be string");

    // Verify TOON format with correct line numbers
    assert!(toon_content.contains("lines[3]{ln,text}:"));
    assert!(toon_content.contains("2,Line 2"));
    assert!(toon_content.contains("3,Line 3"));
    assert!(toon_content.contains("4,Line 4"));
    // Should NOT contain lines 1 or 5
    assert!(!toon_content.contains("1,Line 1"));
    assert!(!toon_content.contains("5,Line 5"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_note_with_invalid_etag_fails() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a note directly
    let note = Note {
        id: String::new(),
        title: "Original Title".to_string(),
        content: "Original content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
    };
    let created = db.notes().create(&note).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Try to edit with wrong etag
    let edit_params = EditNoteParams {
        note_id: created.id.clone(),
        etag: "invalid_etag_12345678901234567890123456789012345678901234567890123456".to_string(),
        title: Some("New Title".to_string()),
        tags: None,
        parent_id: None,
        idx: None,
        repo_ids: None,
        project_ids: None,
        patches: vec![],
    };

    let result = tools.edit_note(Parameters(edit_params)).await;

    // Should fail with etag mismatch error
    assert!(result.is_err(), "edit with invalid etag should fail");
    let err = result.unwrap_err();

    // Print the full error to see what LLM would see
    println!("Error message: {:?}", err);
    println!("Error display: {}", err);

    // Check error contains instruction to re-read
    let err_string = format!("{:?}", err);
    assert!(
        err_string.to_lowercase().contains("re-read"),
        "error should instruct to re-read: {}",
        err_string
    );
}
