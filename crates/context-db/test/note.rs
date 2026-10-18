//! Tests for SqliteNoteRepository.

use crate::SqliteDatabase;
use crate::helpers::count_rows;
use context_core::{Database, HasNotes, Note, NoteQuery, NoteRepository};

fn generate_id() -> String {
    use context_core::generate_entity_id;
    generate_entity_id()
}

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in_memory database");
    db.migrate().expect("Migration should succeed");
    db
}

fn make_note(id: &str, title: &str, content: &str) -> Note {
    Note {
        id: id.to_string(),
        title: title.to_string(),
        content: content.to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],    // Empty by default - relationships managed separately
        project_ids: vec![], // Empty by default - relationships managed separately
        subnote_count: None, // Computed field - not set on creation
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    }
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
    use context_core::NOTE_WARN_SIZE;

    let db = setup_db().await;
    let notes = db.notes();

    // Create note just over warning threshold
    let large_content = "x".repeat(NOTE_WARN_SIZE + 100);
    let note = Note {
        id: "warn0001".to_string(),
        title: "Large Note".to_string(),
        content: large_content.clone(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
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
    use context_core::NOTE_HARD_MAX;

    let db = setup_db().await;
    let notes = db.notes();

    // Create note exactly at hard maximum
    let max_content = "x".repeat(NOTE_HARD_MAX);
    let note = Note {
        id: "max00001".to_string(),
        title: "Maximum Size Note".to_string(),
        content: max_content.clone(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };

    let result = notes.create(&note).await;
    assert!(result.is_ok(), "Note exactly at HARD_MAX should succeed");
}

#[tokio::test(flavor = "multi_thread")]
async fn note_create_over_hard_max_fails() {
    use context_core::NOTE_HARD_MAX;

    let db = setup_db().await;
    let notes = db.notes();

    // Create note over hard maximum
    let oversized_content = "x".repeat(NOTE_HARD_MAX + 1);
    let note = Note {
        id: "over0001".to_string(),
        title: "Oversized Note".to_string(),
        content: oversized_content,
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
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
    use context_core::NOTE_HARD_MAX;

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

#[tokio::test(flavor = "multi_thread")]
async fn note_timestamps_are_optional() {
    let db = setup_db().await;

    // Test 1: Provided timestamps are respected
    let note_with_timestamps = Note {
        id: String::new(),
        title: "Note with timestamps".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-15 10:00:00".to_string()),
        updated_at: Some("2025-01-15 11:00:00".to_string()),
        deleted_at: None,
    };

    let created_with_ts = db
        .notes()
        .create(&note_with_timestamps)
        .await
        .expect("Create note");
    assert_eq!(
        created_with_ts.created_at,
        Some("2025-01-15 10:00:00".to_string())
    );
    assert_eq!(
        created_with_ts.updated_at,
        Some("2025-01-15 11:00:00".to_string())
    );

    // Test 2: None timestamps are auto-generated
    let note_without_timestamps = Note {
        id: String::new(),
        title: "Note without timestamps".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };

    let created_without_ts = db
        .notes()
        .create(&note_without_timestamps)
        .await
        .expect("Create note");
    assert!(created_without_ts.created_at.is_some());
    assert!(created_without_ts.updated_at.is_some());
    assert!(!created_without_ts.created_at.as_ref().unwrap().is_empty());
    assert!(!created_without_ts.updated_at.as_ref().unwrap().is_empty());
}

// =============================================================================
// Transaction Atomicity Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_note_with_invalid_repo_should_rollback() {
    let db = setup_db().await;
    let notes = db.notes();

    // Try to create a note with an invalid repo_id (foreign key violation)
    let note = Note {
        id: generate_id(),
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec!["nonexistent_repo".to_string()], // Invalid repo_id
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };

    let note_id = note.id.clone();
    let result = notes.create(&note).await;

    // Should fail due to foreign key constraint
    assert!(result.is_err(), "Create should fail with invalid repo_id");

    // Verify the note was NOT created (transaction rolled back)
    let get_result = notes.get(&note_id).await;
    assert!(
        get_result.is_err(),
        "Note should not exist after failed transaction"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn create_note_with_invalid_project_should_rollback() {
    let db = setup_db().await;
    let notes = db.notes();

    // Try to create a note with an invalid project_id (foreign key violation)
    let note = Note {
        id: generate_id(),
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec!["nonexistent_project".to_string()], // Invalid project_id
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };

    let note_id = note.id.clone();
    let result = notes.create(&note).await;

    // Should fail due to foreign key constraint
    assert!(
        result.is_err(),
        "Create should fail with invalid project_id"
    );

    // Verify the note was NOT created (transaction rolled back)
    let get_result = notes.get(&note_id).await;
    assert!(
        get_result.is_err(),
        "Note should not exist after failed transaction"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_soft_deleted_returns_not_found() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note("notedel2", "Deleted Note", "Body"))
        .await
        .expect("Create should succeed");
    notes
        .delete("notedel2")
        .await
        .expect("Delete should succeed");

    let result = notes.get("notedel2").await;
    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "get on a soft-deleted note should return NotFound, got: {:?}",
        result
    );

    let metadata = notes.get_metadata_only("notedel2").await;
    assert!(
        matches!(metadata, Err(context_core::DbError::NotFound { .. })),
        "get_metadata_only on a soft-deleted note should return NotFound, got: {:?}",
        metadata
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_excludes_soft_deleted() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note("notelive", "Live Note", "Body"))
        .await
        .expect("Create should succeed");
    notes
        .create(&make_note("notedead", "Dead Note", "Body"))
        .await
        .expect("Create should succeed");
    notes
        .delete("notedead")
        .await
        .expect("Delete should succeed");

    let result = notes.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1, "list should exclude tombstones");
    assert_eq!(result.items[0].id, "notelive");
    assert_eq!(result.total, 1, "list total should exclude tombstones");
    assert_eq!(
        notes.count().await.expect("Count should succeed"),
        1,
        "count should exclude tombstones"
    );

    let metadata = notes
        .list_metadata_only(None)
        .await
        .expect("Metadata list should succeed");
    assert_eq!(
        metadata.items.len(),
        1,
        "metadata list should exclude tombstones"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_subnote_count_excludes_soft_deleted() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note("notepar2", "Parent", "Body"))
        .await
        .expect("Create parent should succeed");

    let mut child = make_note("notechi2", "Child", "Body");
    child.parent_id = Some("notepar2".to_string());
    notes
        .create(&child)
        .await
        .expect("Create child should succeed");

    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };

    let result = notes.list(Some(&query)).await.expect("List should succeed");
    let parent = result
        .items
        .iter()
        .find(|note| note.id == "notepar2")
        .expect("parent should be listed");
    assert_eq!(parent.subnote_count, Some(1));

    notes
        .delete("notechi2")
        .await
        .expect("Delete child should succeed");

    let result = notes.list(Some(&query)).await.expect("List should succeed");
    let parent = result
        .items
        .iter()
        .find(|note| note.id == "notepar2")
        .expect("parent should be listed");
    assert_eq!(
        parent.subnote_count,
        Some(0),
        "soft-deleted subnotes must not be counted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn hard_delete_after_soft_delete_does_not_corrupt_fts() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note("notehard", "Zebra Hard Delete", "Body"))
        .await
        .expect("Create should succeed");
    notes
        .delete("notehard")
        .await
        .expect("Soft delete should succeed");

    // Simulate a Phase 4 purge: hard-delete the tombstone directly.
    sqlx::query("DELETE FROM note WHERE id = ?")
        .bind("notehard")
        .execute(db.pool())
        .await
        .expect("Hard delete should succeed");

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM note_fts WHERE note_fts MATCH 'Zebra'")
            .fetch_one(db.pool())
            .await
            .expect("FTS query should succeed after hard delete");
    assert_eq!(count, 0);
}

// =============================================================================
// Tombstone purge tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_removes_old_tombstones() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note("noteprg1", "Old Tombstone", "Body"))
        .await
        .unwrap();
    notes.delete("noteprg1").await.unwrap();
    sqlx::query("UPDATE note SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("noteprg1")
        .execute(db.pool())
        .await
        .unwrap();

    notes
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "note", "id = ?", "noteprg1").await,
        0,
        "an old note tombstone must be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_preserves_recent_tombstones() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note("noteprg2", "Recent Tombstone", "Body"))
        .await
        .unwrap();
    notes.delete("noteprg2").await.unwrap();

    notes
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "note", "id = ?", "noteprg2").await,
        1,
        "a tombstone inside the retention period must be kept"
    );
}
