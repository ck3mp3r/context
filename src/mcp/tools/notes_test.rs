//! Tests for Note MCP tools

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
    let tools = NoteTools::new(db.clone());

    let params = ListNotesParams {
        tags: None,
        note_type: None,
        limit: None,
        offset: None,
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
    let tools = NoteTools::new(db.clone());

    // Create note
    let create_params = CreateNoteParams {
        title: "Meeting Notes".to_string(),
        content: "# Meeting with team\n\n- Discussed project timeline\n- Assigned tasks"
            .to_string(),
        tags: Some(vec!["meeting".to_string(), "team".to_string()]),
        note_type: None,
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
    let tools = NoteTools::new(db.clone());

    let params = GetNoteParams {
        note_id: "nonexist".to_string(),
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
        repo_ids: vec![],
        project_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    let note2 = Note {
        id: String::new(),
        title: "Personal Note".to_string(),
        content: "Content 2".to_string(),
        tags: vec!["personal".to_string()],
        note_type: NoteType::Manual,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();

    let tools = NoteTools::new(db.clone());

    // List only "work" notes
    let params = ListNotesParams {
        tags: Some(vec!["work".to_string()]),
        note_type: None,
        limit: None,
        offset: None,
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
        repo_ids: vec![],
        project_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    let created = db.notes().create(&note).await.unwrap();

    let tools = NoteTools::new(db.clone());

    // Update note
    let update_params = UpdateNoteParams {
        note_id: created.id.clone(),
        title: Some("Updated Title".to_string()),
        content: Some("Updated content with more details".to_string()),
        tags: Some(vec!["updated".to_string()]),
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
        repo_ids: vec![],
        project_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    let created = db.notes().create(&note).await.unwrap();

    let tools = NoteTools::new(db.clone());

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
        repo_ids: vec![],
        project_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    let note2 = Note {
        id: String::new(),
        title: "Python Tutorial".to_string(),
        content: "Python list comprehensions and generators".to_string(),
        tags: vec![],
        note_type: NoteType::Manual,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();

    let tools = NoteTools::new(db.clone());

    // Search for "Rust"
    let params = SearchNotesParams {
        query: "Rust".to_string(),
        tags: None,
        limit: None,
        offset: None,
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
        repo_ids: vec![],
        project_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    let note2 = Note {
        id: String::new(),
        title: "Rust Basics".to_string(),
        content: "Basic Rust syntax and types".to_string(),
        tags: vec!["rust".to_string(), "basics".to_string()],
        note_type: NoteType::Manual,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    db.notes().create(&note1).await.unwrap();
    db.notes().create(&note2).await.unwrap();

    let tools = NoteTools::new(db.clone());

    // Search for "Rust" with "async" tag filter
    let params = SearchNotesParams {
        query: "Rust".to_string(),
        tags: Some(vec!["async".to_string()]),
        limit: None,
        offset: None,
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
