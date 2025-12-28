//! Tests for SqliteNoteRepository.

use crate::db::{Database, Note, NoteQuery, NoteRepository, NoteType, SqliteDatabase};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

fn make_note(id: &str, title: &str, content: &str) -> Note {
    Note {
        id: id.to_string(),
        title: title.to_string(),
        content: content.to_string(),
        tags: vec![],
        note_type: NoteType::Manual,
        repo_ids: vec![],    // Empty by default - relationships managed separately
        project_ids: vec![], // Empty by default - relationships managed separately
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn note_create_and_get() {
    let db = setup_db().await;
    let notes = db.notes();

    let note = Note {
        id: "note0001".to_string(),
        title: "My First Note".to_string(),
        content: "This is markdown content\n\n## Heading\n\nWith paragraphs.".to_string(),
        tags: vec!["session".to_string(), "important".to_string()],
        note_type: NoteType::Manual,
        repo_ids: vec![],    // Empty by default - relationships managed separately
        project_ids: vec![], // Empty by default - relationships managed separately
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };

    notes.create(&note).await.expect("Create should succeed");

    let retrieved = notes.get("note0001").await.expect("Get should succeed");
    assert_eq!(retrieved.id, note.id);
    assert_eq!(retrieved.title, note.title);
    assert_eq!(retrieved.content, note.content);
    assert_eq!(retrieved.tags, note.tags);
    assert_eq!(retrieved.note_type, NoteType::Manual);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_get_nonexistent_returns_not_found() {
    let db = setup_db().await;
    let notes = db.notes();

    let result = notes.get("nonexist").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn note_list() {
    let db = setup_db().await;
    let notes = db.notes();

    // Initially empty
    let result = notes.list(None).await.expect("List should succeed");
    assert!(result.items.is_empty());

    // Add notes
    notes
        .create(&make_note("noteaaa1", "First", "Content one"))
        .await
        .unwrap();
    notes
        .create(&make_note("notebbb2", "Second", "Content two"))
        .await
        .unwrap();

    let result = notes.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_update() {
    let db = setup_db().await;
    let notes = db.notes();

    let mut note = make_note("noteupd1", "Original Title", "Original content");
    notes.create(&note).await.expect("Create should succeed");

    note.title = "Updated Title".to_string();
    note.content = "Updated content with more text".to_string();
    note.tags = vec!["updated".to_string()];
    note.note_type = NoteType::ArchivedTodo;
    notes.update(&note).await.expect("Update should succeed");

    let retrieved = notes.get("noteupd1").await.expect("Get should succeed");
    assert_eq!(retrieved.title, "Updated Title");
    assert_eq!(retrieved.content, "Updated content with more text");
    assert_eq!(retrieved.tags, vec!["updated".to_string()]);
    assert_eq!(retrieved.note_type, NoteType::ArchivedTodo);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_delete() {
    let db = setup_db().await;
    let notes = db.notes();

    let note = make_note("notedel1", "To Delete", "Will be deleted");
    notes.create(&note).await.expect("Create should succeed");

    notes
        .delete("notedel1")
        .await
        .expect("Delete should succeed");

    let result = notes.get("notedel1").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn note_search() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes with specific content
    notes
        .create(&make_note(
            "notesrc1",
            "API Design",
            "REST endpoints for user management",
        ))
        .await
        .unwrap();
    notes
        .create(&make_note(
            "notesrc2",
            "Database Schema",
            "SQLite tables for user data",
        ))
        .await
        .unwrap();
    notes
        .create(&make_note(
            "notesrc3",
            "Frontend Guide",
            "React components for dashboard",
        ))
        .await
        .unwrap();

    // Search for "user" - should find 2 notes
    let results = notes
        .search("user", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 2);

    // Search for "React" - should find 1 note
    let results = notes
        .search("React", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].title, "Frontend Guide");

    // Search for nonexistent term
    let results = notes
        .search("kubernetes", None)
        .await
        .expect("Search should succeed");
    assert!(results.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn note_list_with_tag_filter() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes with different tags
    let mut note1 = make_note("notetag1", "Rust Guide", "About Rust");
    note1.tags = vec!["rust".to_string(), "programming".to_string()];
    notes.create(&note1).await.unwrap();

    let mut note2 = make_note("notetag2", "Python Guide", "About Python");
    note2.tags = vec!["python".to_string(), "programming".to_string()];
    notes.create(&note2).await.unwrap();

    let mut note3 = make_note("notetag3", "Cooking Recipe", "About cooking");
    note3.tags = vec!["cooking".to_string()];
    notes.create(&note3).await.unwrap();

    // Filter by "rust" tag - should find 1
    let query = NoteQuery {
        tags: Some(vec!["rust".to_string()]),
        ..Default::default()
    };
    let results = notes.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].title, "Rust Guide");

    // Filter by "programming" tag - should find 2
    let query = NoteQuery {
        tags: Some(vec!["programming".to_string()]),
        ..Default::default()
    };
    let results = notes.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(results.items.len(), 2);
    assert_eq!(results.total, 2);

    // Filter by nonexistent tag
    let query = NoteQuery {
        tags: Some(vec!["nonexistent".to_string()]),
        ..Default::default()
    };
    let results = notes.list(Some(&query)).await.expect("List should succeed");
    assert!(results.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn note_search_with_tag_filter() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create notes with different tags and content
    let mut note1 = make_note("srctag01", "API Design", "REST API patterns");
    note1.tags = vec!["api".to_string(), "backend".to_string()];
    notes.create(&note1).await.unwrap();

    let mut note2 = make_note("srctag02", "API Testing", "Testing API endpoints");
    note2.tags = vec!["api".to_string(), "testing".to_string()];
    notes.create(&note2).await.unwrap();

    let mut note3 = make_note("srctag03", "Frontend APIs", "Calling APIs from React");
    note3.tags = vec!["frontend".to_string()];
    notes.create(&note3).await.unwrap();

    // Search for "API" with no tag filter - should find all 3
    let results = notes
        .search("API", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 3);

    // Search for "API" with "backend" tag filter - should find 1
    let query = NoteQuery {
        tags: Some(vec!["backend".to_string()]),
        ..Default::default()
    };
    let results = notes
        .search("API", Some(&query))
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].title, "API Design");

    // Search for "API" with "api" tag filter - should find 2
    let query = NoteQuery {
        tags: Some(vec!["api".to_string()]),
        ..Default::default()
    };
    let results = notes
        .search("API", Some(&query))
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 2);
    assert_eq!(results.total, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_get_loads_repo_and_project_relationships() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create repos first (for foreign key constraints)
    sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind("repo0001")
        .bind("https://github.com/test/repo1")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert repo1 should succeed");

    sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind("repo0002")
        .bind("https://github.com/test/repo2")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert repo2 should succeed");

    // Create project first (for foreign key constraints)
    sqlx::query("INSERT INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("proj0001")
        .bind("Test Project")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert project should succeed");

    // Create a note
    let note = make_note("reltest1", "Test Note", "Content");
    notes.create(&note).await.expect("Create should succeed");

    // Insert relationships into junction tables
    sqlx::query("INSERT INTO note_repo (note_id, repo_id) VALUES (?, ?)")
        .bind("reltest1")
        .bind("repo0001")
        .execute(db.pool())
        .await
        .expect("Insert note_repo should succeed");

    sqlx::query("INSERT INTO note_repo (note_id, repo_id) VALUES (?, ?)")
        .bind("reltest1")
        .bind("repo0002")
        .execute(db.pool())
        .await
        .expect("Insert note_repo should succeed");

    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("proj0001")
        .bind("reltest1")
        .execute(db.pool())
        .await
        .expect("Insert project_note should succeed");

    // Get note and verify relationships are loaded
    let retrieved = notes.get("reltest1").await.expect("Get should succeed");

    assert_eq!(
        retrieved.repo_ids.len(),
        2,
        "Should load 2 repo relationships"
    );
    assert!(retrieved.repo_ids.contains(&"repo0001".to_string()));
    assert!(retrieved.repo_ids.contains(&"repo0002".to_string()));

    assert_eq!(
        retrieved.project_ids.len(),
        1,
        "Should load 1 project relationship"
    );
    assert!(retrieved.project_ids.contains(&"proj0001".to_string()));
}

// =============================================================================
// Size Validation Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn note_create_with_warn_size_content_succeeds_with_warning() {
    use crate::db::models::NOTE_WARN_SIZE;

    let db = setup_db().await;
    let notes = db.notes();

    // Create note just over warning threshold
    let large_content = "x".repeat(NOTE_WARN_SIZE + 100);
    let note = Note {
        id: "warn0001".to_string(),
        title: "Large Note".to_string(),
        content: large_content.clone(),
        tags: vec![],
        note_type: NoteType::Manual,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };

    // Should succeed (warn size is advisory, not enforced)
    let result = notes.create(&note).await;
    assert!(
        result.is_ok(),
        "Note over WARN_SIZE should succeed (warning is advisory)"
    );

    let retrieved = notes.get("warn0001").await.expect("Should retrieve note");
    assert_eq!(retrieved.content.len(), large_content.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn note_create_at_hard_max_succeeds() {
    use crate::db::models::NOTE_HARD_MAX;

    let db = setup_db().await;
    let notes = db.notes();

    // Create note exactly at hard maximum
    let max_content = "x".repeat(NOTE_HARD_MAX);
    let note = Note {
        id: "max00001".to_string(),
        title: "Maximum Size Note".to_string(),
        content: max_content.clone(),
        tags: vec![],
        note_type: NoteType::Manual,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };

    let result = notes.create(&note).await;
    assert!(result.is_ok(), "Note exactly at HARD_MAX should succeed");
}

#[tokio::test(flavor = "multi_thread")]
async fn note_create_over_hard_max_fails() {
    use crate::db::models::NOTE_HARD_MAX;

    let db = setup_db().await;
    let notes = db.notes();

    // Create note over hard maximum
    let oversized_content = "x".repeat(NOTE_HARD_MAX + 1);
    let note = Note {
        id: "over0001".to_string(),
        title: "Oversized Note".to_string(),
        content: oversized_content,
        tags: vec![],
        note_type: NoteType::Manual,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };

    let result = notes.create(&note).await;
    assert!(result.is_err(), "Note over HARD_MAX should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("exceeds maximum size") || err_msg.contains("too large"),
        "Error message should mention size limit, got: {}",
        err_msg
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_update_over_hard_max_fails() {
    use crate::db::models::NOTE_HARD_MAX;

    let db = setup_db().await;
    let notes = db.notes();

    // Create normal note
    let note = make_note("upd00001", "Normal Note", "Initial content");
    notes.create(&note).await.expect("Create should succeed");

    // Try to update with oversized content
    let oversized_content = "x".repeat(NOTE_HARD_MAX + 1);
    let mut updated_note = note.clone();
    updated_note.content = oversized_content;

    let result = notes.update(&updated_note).await;
    assert!(result.is_err(), "Update over HARD_MAX should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("exceeds maximum size") || err_msg.contains("too large"),
        "Error message should mention size limit, got: {}",
        err_msg
    );
}

// =============================================================================
// Metadata-Only Retrieval Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn note_get_with_content_excluded() {
    let db = setup_db().await;
    let notes = db.notes();

    let note = make_note(
        "meta0001",
        "Test Note",
        "This is the content that should be excluded",
    );
    notes.create(&note).await.expect("Create should succeed");

    // Get note without content
    let retrieved = notes
        .get_metadata_only("meta0001")
        .await
        .expect("Get should succeed");

    assert_eq!(retrieved.id, "meta0001");
    assert_eq!(retrieved.title, "Test Note");
    assert_eq!(
        retrieved.content, "",
        "Content should be empty when excluded"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_list_with_content_excluded() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create multiple notes
    let note1 = make_note("list0001", "Note 1", "Content 1 - should be excluded");
    let note2 = make_note("list0002", "Note 2", "Content 2 - should be excluded");
    notes.create(&note1).await.expect("Create should succeed");
    notes.create(&note2).await.expect("Create should succeed");

    // List notes without content
    let result = notes
        .list_metadata_only(None)
        .await
        .expect("List should succeed");

    assert_eq!(result.items.len(), 2);
    for item in &result.items {
        assert_eq!(item.content, "", "Content should be empty for all items");
        assert!(!item.title.is_empty(), "Title should still be present");
    }
}
