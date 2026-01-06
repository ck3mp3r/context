//! Tests for Note MCP tools

use crate::api::notifier::ChangeNotifier;
use crate::db::{Database, Note, NoteRepository, NoteType, SqliteDatabase};
use crate::mcp::tools::notes::{
    CreateNoteParams, DeleteNoteParams, GetNoteParams, ListNotesParams, NoteTools,
    SearchNotesParams, UpdateNoteParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::RawContent;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_empty() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let params = ListNotesParams {
        tags: None,
        note_type: None,
        project_id: None,
        parent_id: None,
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
async fn test_create_and_get_note() {
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
        note_type: None,
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
    assert_eq!(created.note_type, NoteType::Manual);
    assert!(!created.id.is_empty());

    // Get the note
    let get_params = GetNoteParams {
        note_id: created.id.clone(),
        include_content: None, // Default to true
    };

    let result = tools
        .get_note(Parameters(get_params))
        .await
        .expect("get should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let retrieved: Note = serde_json::from_str(content_text).unwrap();

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.title, "Meeting Notes");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_note_not_found() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    let params = GetNoteParams {
        note_id: "nonexist".to_string(),
        include_content: None,
    };

    let result = tools.get_note(Parameters(params)).await;
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
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    let note2 = Note {
        id: String::new(),
        title: "Personal Note".to_string(),
        content: "Content 2".to_string(),
        tags: vec!["personal".to_string()],
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // List only "work" notes
    let params = ListNotesParams {
        tags: Some(vec!["work".to_string()]),
        note_type: None,
        project_id: None,
        parent_id: None,
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
async fn test_update_note() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a note
    let note = Note {
        id: String::new(),
        title: "Original Title".to_string(),
        content: "Original content".to_string(),
        tags: vec![],
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    let created = db.notes().create(&note).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Update note
    let update_params = UpdateNoteParams {
        note_id: created.id.clone(),
        title: Some("Updated Title".to_string()),
        content: Some("Updated content with more details".to_string()),
        tags: Some(vec!["updated".to_string()]),
        parent_id: None,
        idx: None,
        repo_ids: None,
        project_ids: None,
    };

    let result = tools
        .update_note(Parameters(update_params))
        .await
        .expect("update should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let updated: Note = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.title, "Updated Title");
    assert_eq!(updated.content, "Updated content with more details");
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
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
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
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    let note2 = Note {
        id: String::new(),
        title: "Python Tutorial".to_string(),
        content: "Python list comprehensions and generators".to_string(),
        tags: vec![],
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Search for "Rust"
    let params = SearchNotesParams {
        query: "Rust".to_string(),
        tags: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .search_notes(Parameters(params))
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
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    let note2 = Note {
        id: String::new(),
        title: "Rust Basics".to_string(),
        content: "Basic Rust syntax and types".to_string(),
        tags: vec!["rust".to_string(), "basics".to_string()],
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Search for "Rust" with "async" tag filter
    let params = SearchNotesParams {
        query: "Rust".to_string(),
        tags: Some(vec!["async".to_string()]),
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .search_notes(Parameters(params))
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
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: Some("2025-01-01 10:00:00".to_string()),
        updated_at: Some("2025-01-01 10:00:00".to_string()),
    };

    let note2 = Note {
        id: String::new(),
        title: "Second Note".to_string(),
        content: "Second content".to_string(),
        tags: vec![],
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: Some("2025-01-02 10:00:00".to_string()),
        updated_at: Some("2025-01-03 10:00:00".to_string()),
    };

    let note3 = Note {
        id: String::new(),
        title: "Third Note".to_string(),
        content: "Third content".to_string(),
        tags: vec![],
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: Some("2025-01-03 10:00:00".to_string()),
        updated_at: Some("2025-01-02 10:00:00".to_string()),
    };

    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();
    db.notes().create(&note3).await.unwrap();

    let tools = NoteTools::new(db.clone(), ChangeNotifier::new());

    // Test sorting by updated_at DESC
    let params = ListNotesParams {
        note_type: None,
        tags: None,
        project_id: None,
        parent_id: None,
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
        note_type: None,
        tags: None,
        project_id: None,
        parent_id: None,
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
        note_type: None,
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
        note_type: None,
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
        note_type: None,
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

    // Update idx
    let update_params = UpdateNoteParams {
        note_id: created.id.clone(),
        title: Some("Test Note".to_string()),
        content: Some("Content".to_string()),
        tags: None,
        parent_id: None,
        idx: Some(Some(20)),
        repo_ids: None,
        project_ids: None,
    };

    let update_result = tools
        .update_note(Parameters(update_params))
        .await
        .expect("update should succeed");

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
        note_type: None,
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
            note_type: None,
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
        tags: None,
        note_type: None,
        project_id: None,
        parent_id: Some(parent.id.clone()),
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
