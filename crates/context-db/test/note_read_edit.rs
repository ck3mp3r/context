//! Tests for SqliteNoteRepository.

use crate::SqliteDatabase;
use context_core::{Database, HasNotes, Note, NoteRepository};

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

// =============================================================================
// Line Range Reading Tests (TDD - RED)
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_with_line_ranges() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create a note with multiple lines
    let note = Note {
        id: generate_id(),
        title: "Multi-line Note".to_string(),
        content: "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10"
            .to_string(),
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
    let created = notes.create(&note).await.unwrap();

    // Define line ranges to fetch: lines 1-3 and lines 7-9
    let ranges = vec![(1, 3), (7, 9)];

    // Call the repository function (this will fail - RED phase)
    let result = notes.get_line_ranges(&created.id, &ranges).await;

    // Expected result: vec!["Line 1\nLine 2\nLine 3", "Line 7\nLine 8\nLine 9"]
    assert!(result.is_ok(), "Should successfully read line ranges");
    let line_groups = result.unwrap();
    assert_eq!(line_groups.len(), 2, "Should return 2 line groups");
    assert_eq!(line_groups[0], "Line 1\nLine 2\nLine 3");
    assert_eq!(line_groups[1], "Line 7\nLine 8\nLine 9");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_read_note_with_overlapping_ranges_fails() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create a note
    let note = Note {
        id: generate_id(),
        title: "Test Note".to_string(),
        content: "Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string(),
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
    let created = notes.create(&note).await.unwrap();

    // Define overlapping ranges: lines 1-3 and lines 2-4 (overlap!)
    let ranges = vec![(1, 3), (2, 4)];

    // Call should fail with validation error
    let result = notes.get_line_ranges(&created.id, &ranges).await;

    assert!(result.is_err(), "Should fail with overlapping ranges");
    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("overlap") || err_msg.contains("Overlapping"),
        "Error should mention overlapping ranges, got: {}",
        err_msg
    );
}

// =============================================================================
// Line Range Editing Tests (TDD - RED)
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_note_with_line_range_patches() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create a note with multiple lines
    let note = Note {
        id: generate_id(),
        title: "Multi-line Note".to_string(),
        content: "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10"
            .to_string(),
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
    let created = notes.create(&note).await.unwrap();

    // Define patches: replace lines 2-3 with new content, and lines 7-8 with other content
    let patches = vec![
        ((2, 3), "REPLACED 2-3".to_string()),
        ((7, 8), "REPLACED 7-8".to_string()),
    ];

    // Apply patches (should be applied in reverse order internally)
    let result = notes.apply_line_patches(&created.id, &patches).await;

    assert!(result.is_ok(), "Should successfully apply line patches");

    // Verify the result
    let updated = notes.get(&created.id).await.unwrap();
    let expected = "Line 1\nREPLACED 2-3\nLine 4\nLine 5\nLine 6\nREPLACED 7-8\nLine 9\nLine 10";
    assert_eq!(updated.content, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_note_with_overlapping_patches_fails() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create a note
    let note = Note {
        id: generate_id(),
        title: "Test Note".to_string(),
        content: "Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string(),
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
    let created = notes.create(&note).await.unwrap();

    // Define overlapping patches: lines 1-2 and lines 2-3 (overlap!)
    let patches = vec![
        ((1, 2), "PATCH 1".to_string()),
        ((2, 3), "PATCH 2".to_string()),
    ];

    // Call should fail with validation error
    let result = notes.apply_line_patches(&created.id, &patches).await;

    assert!(result.is_err(), "Should fail with overlapping patches");
    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("overlap") || err_msg.contains("Overlapping"),
        "Error should mention overlapping ranges, got: {}",
        err_msg
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_edit_applies_patches_in_reverse_order() {
    let db = setup_db().await;
    let notes = db.notes();

    // Create a note
    let note = Note {
        id: generate_id(),
        title: "Test Note".to_string(),
        content: "AAA\nBBB\nCCC\nDDD\nEEE".to_string(),
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
    let created = notes.create(&note).await.unwrap();

    // Define patches in non-reverse order (tool should sort and apply in reverse)
    // Replace line 2 with "X" and line 4 with "Y"
    let patches = vec![((2, 2), "X".to_string()), ((4, 4), "Y".to_string())];

    let result = notes.apply_line_patches(&created.id, &patches).await;
    assert!(result.is_ok(), "Should apply patches successfully");

    // Expected: line 4 replaced first (reverse order), then line 2
    // This ensures line numbers stay accurate
    let updated = notes.get(&created.id).await.unwrap();
    let expected = "AAA\nX\nCCC\nY\nEEE";
    assert_eq!(updated.content, expected);
}
