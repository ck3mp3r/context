//! Tests for SqliteNoteRepository.

use crate::SqliteDatabase;
use context_core::{Database, HasNotes, Note, NoteQuery, NoteRepository, PageSort, SortOrder};

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
// Hierarchical Notes (parent_id and idx)
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_note_with_parent_id() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create parent note
    let parent = Note {
        id: generate_id(),
        title: "Parent Note".to_string(),
        content: "Parent content".to_string(),
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
    let created_parent = db.notes().create(&parent).await.unwrap();

    // Create child note
    let child = Note {
        id: generate_id(),
        title: "Child Note".to_string(),
        content: "Child content".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    let created_child = db.notes().create(&child).await.unwrap();

    // Verify parent_id is set
    assert_eq!(created_child.parent_id, Some(created_parent.id));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_subnote_with_idx() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create parent note
    let parent = Note {
        id: generate_id(),
        title: "Parent Note".to_string(),
        content: "Parent content".to_string(),
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
    let created_parent = db.notes().create(&parent).await.unwrap();

    // Create child note with idx
    let child = Note {
        id: generate_id(),
        title: "Child Note".to_string(),
        content: "Child content".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: Some(10),
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    let created_child = db.notes().create(&child).await.unwrap();

    // Verify idx is set
    assert_eq!(created_child.idx, Some(10));
    assert_eq!(created_child.parent_id, Some(created_parent.id));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_subnotes_ordered_by_idx() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create parent note
    let parent = Note {
        id: generate_id(),
        title: "Parent Note".to_string(),
        content: "Parent content".to_string(),
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
    let created_parent = db.notes().create(&parent).await.unwrap();

    // Create multiple child notes with different idx values
    let child1 = Note {
        id: generate_id(),
        title: "Child 1 (idx=30)".to_string(),
        content: "Should be third".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: Some(30),
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child1).await.unwrap();

    let child2 = Note {
        id: generate_id(),
        title: "Child 2 (idx=10)".to_string(),
        content: "Should be first".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: Some(10),
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child2).await.unwrap();

    let child3 = Note {
        id: generate_id(),
        title: "Child 3 (idx=20)".to_string(),
        content: "Should be second".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: Some(20),
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child3).await.unwrap();

    // Query for subnotes filtered by parent_id, sorted by idx
    let query = NoteQuery {
        parent_id: Some(created_parent.id.clone()),
        page: PageSort {
            sort_by: Some("idx".to_string()),
            sort_order: Some(SortOrder::Asc),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();

    // Verify ordering by idx
    assert_eq!(result.items.len(), 3);
    assert_eq!(result.items[0].title, "Child 2 (idx=10)");
    assert_eq!(result.items[1].title, "Child 3 (idx=20)");
    assert_eq!(result.items[2].title, "Child 1 (idx=30)");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_note_type_filter_returns_only_parent_notes() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 2 parent notes
    let parent1 = Note {
        id: generate_id(),
        title: "Parent 1".to_string(),
        content: "Parent content 1".to_string(),
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
    let created_parent1 = db.notes().create(&parent1).await.unwrap();

    let parent2 = Note {
        id: generate_id(),
        title: "Parent 2".to_string(),
        content: "Parent content 2".to_string(),
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
    db.notes().create(&parent2).await.unwrap();

    // Create 2 subnotes
    let child1 = Note {
        id: generate_id(),
        title: "Child 1".to_string(),
        content: "Child content 1".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child1).await.unwrap();

    let child2 = Note {
        id: generate_id(),
        title: "Child 2".to_string(),
        content: "Child content 2".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child2).await.unwrap();

    // Test filter for parent notes only (note_type="note")
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return only 2 parent notes");
    assert_eq!(result.items.len(), 2);
    for note in &result.items {
        assert!(
            note.parent_id.is_none(),
            "All notes should have parent_id = None"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_note_type_filter_returns_only_subnotes() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 1 parent note
    let parent = Note {
        id: generate_id(),
        title: "Parent".to_string(),
        content: "Parent content".to_string(),
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
    let created_parent = db.notes().create(&parent).await.unwrap();

    // Create 2 subnotes
    let child1 = Note {
        id: generate_id(),
        title: "Child 1".to_string(),
        content: "Child content 1".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child1).await.unwrap();

    let child2 = Note {
        id: generate_id(),
        title: "Child 2".to_string(),
        content: "Child content 2".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child2).await.unwrap();

    // Test filter for subnotes only (note_type="subnote")
    let query = NoteQuery {
        note_type: Some("subnote".to_string()),
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return only 2 subnotes");
    assert_eq!(result.items.len(), 2);
    for note in &result.items {
        assert!(
            note.parent_id.is_some(),
            "All notes should have parent_id != None"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_with_note_type_filter_returns_only_parent_notes() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 2 parent notes with searchable content
    let parent1 = Note {
        id: generate_id(),
        title: "Parent Note 1".to_string(),
        content: "Searchable content in parent note".to_string(),
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
    let created_parent1 = db.notes().create(&parent1).await.unwrap();

    let parent2 = Note {
        id: generate_id(),
        title: "Parent Note 2".to_string(),
        content: "More searchable content here".to_string(),
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
    db.notes().create(&parent2).await.unwrap();

    // Create 2 subnotes with searchable content
    let child1 = Note {
        id: generate_id(),
        title: "Subnote 1".to_string(),
        content: "Searchable content in subnote".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child1).await.unwrap();

    let child2 = Note {
        id: generate_id(),
        title: "Subnote 2".to_string(),
        content: "Another searchable subnote".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child2).await.unwrap();

    // Search with note_type=note filter (should return only parent notes)
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };

    let result = db.notes().search("searchable", Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return only 2 parent notes");
    assert_eq!(result.items.len(), 2);
    for note in &result.items {
        assert!(
            note.parent_id.is_none(),
            "All notes should have parent_id = None"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_with_note_type_filter_returns_only_subnotes() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 1 parent note with searchable content
    let parent = Note {
        id: generate_id(),
        title: "Parent Note".to_string(),
        content: "Searchable content in parent".to_string(),
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
    let created_parent = db.notes().create(&parent).await.unwrap();

    // Create 2 subnotes with searchable content
    let child1 = Note {
        id: generate_id(),
        title: "Subnote 1".to_string(),
        content: "Searchable content in subnote".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child1).await.unwrap();

    let child2 = Note {
        id: generate_id(),
        title: "Subnote 2".to_string(),
        content: "Another searchable subnote".to_string(),
        tags: vec![],
        parent_id: Some(created_parent.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child2).await.unwrap();

    // Search with note_type=subnote filter (should return only subnotes)
    let query = NoteQuery {
        note_type: Some("subnote".to_string()),
        ..Default::default()
    };

    let result = db.notes().search("searchable", Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return only 2 subnotes");
    assert_eq!(result.items.len(), 2);
    for note in &result.items {
        assert!(
            note.parent_id.is_some(),
            "All notes should have parent_id != None"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_with_parent_id_filter() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 2 parent notes
    let parent1 = Note {
        id: generate_id(),
        title: "Parent 1".to_string(),
        content: "Parent 1 content".to_string(),
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
    let created_parent1 = db.notes().create(&parent1).await.unwrap();

    let parent2 = Note {
        id: generate_id(),
        title: "Parent 2".to_string(),
        content: "Parent 2 content".to_string(),
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
    let created_parent2 = db.notes().create(&parent2).await.unwrap();

    // Create subnotes for parent1 with searchable content
    let child1_1 = Note {
        id: generate_id(),
        title: "Child 1.1".to_string(),
        content: "Findme in parent1 child".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child1_1).await.unwrap();

    let child1_2 = Note {
        id: generate_id(),
        title: "Child 1.2".to_string(),
        content: "Findme in another parent1 child".to_string(),
        tags: vec![],
        parent_id: Some(created_parent1.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child1_2).await.unwrap();

    // Create subnote for parent2 with searchable content
    let child2_1 = Note {
        id: generate_id(),
        title: "Child 2.1".to_string(),
        content: "Findme in parent2 child".to_string(),
        tags: vec![],
        parent_id: Some(created_parent2.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&child2_1).await.unwrap();

    // Search with parent_id filter (should return only subnotes of parent1)
    let query = NoteQuery {
        parent_id: Some(created_parent1.id.clone()),
        ..Default::default()
    };

    let result = db.notes().search("Findme", Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return only 2 subnotes of parent1");
    assert_eq!(result.items.len(), 2);
    for note in &result.items {
        assert_eq!(
            note.parent_id.as_ref().unwrap(),
            &created_parent1.id,
            "All notes should have parent_id = parent1.id"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_parent_notes_sorted_by_last_activity() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 3 parent notes
    // We rely on actual timestamps since triggers override passed timestamps
    let parent1 = Note {
        id: generate_id(),
        title: "Parent 1".to_string(),
        content: "First parent (oldest)".to_string(),
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
    let _created_parent1 = db.notes().create(&parent1).await.unwrap();

    // Wait to ensure timestamp difference (SQLite has second precision by default)
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let parent2 = Note {
        id: generate_id(),
        title: "Parent 2".to_string(),
        content: "Second parent".to_string(),
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
    let created_parent2 = db.notes().create(&parent2).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let parent3 = Note {
        id: generate_id(),
        title: "Parent 3".to_string(),
        content: "Third parent".to_string(),
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
    let created_parent3 = db.notes().create(&parent3).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Create a subnote for parent2 - will be updated to have most recent activity
    let subnote = Note {
        id: generate_id(),
        title: "Subnote of Parent 2".to_string(),
        content: "Medium activity".to_string(),
        tags: vec![],
        parent_id: Some(created_parent2.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    let created_subnote = db.notes().create(&subnote).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Update parent3 directly - should have second most recent activity
    let mut updated_parent3 = created_parent3.clone();
    updated_parent3.content = "Updated parent 3".to_string();
    db.notes().update(&updated_parent3).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Update the subnote - parent2 should now have the most recent activity
    let mut updated_subnote = created_subnote.clone();
    updated_subnote.content = "Latest activity".to_string();
    db.notes().update(&updated_subnote).await.unwrap();

    // Query parent notes with type=note filter (no explicit sort)
    // Expected order by last_activity_at DESC:
    // 1. Parent 2 (most recent - subnote updated last)
    // 2. Parent 3 (medium - updated directly)
    // 3. Parent 1 (oldest - no updates)
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();
    assert_eq!(result.total, 3, "Should return all 3 parent notes");
    assert_eq!(result.items.len(), 3);

    // Verify sort order by last activity (DESC)
    assert_eq!(
        result.items[0].title, "Parent 2",
        "Parent 2 should be first (most recent activity via subnote)"
    );
    assert_eq!(
        result.items[1].title, "Parent 3",
        "Parent 3 should be second (updated directly)"
    );
    assert_eq!(
        result.items[2].title, "Parent 1",
        "Parent 1 should be third (oldest, no updates)"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_parent_notes_explicit_sort_overrides_activity_sort() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

    // Create 2 parent notes
    let parent_a = Note {
        id: generate_id(),
        title: "Z Parent".to_string(),
        content: "Should be last alphabetically".to_string(),
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
    let created_parent_a = db.notes().create(&parent_a).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let parent_b = Note {
        id: generate_id(),
        title: "A Parent".to_string(),
        content: "Should be first alphabetically".to_string(),
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
    db.notes().create(&parent_b).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Create subnote for Z Parent - makes it most recently active
    let subnote = Note {
        id: generate_id(),
        title: "Subnote".to_string(),
        content: "Latest activity".to_string(),
        tags: vec![],
        parent_id: Some(created_parent_a.id.clone()),
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&subnote).await.unwrap();

    // Query with explicit sort by title ASC
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        page: context_core::PageSort {
            sort_by: Some("title".to_string()),
            sort_order: Some(context_core::SortOrder::Asc),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return 2 parent notes");
    assert_eq!(result.items.len(), 2);

    // Verify sort order is by title ASC, NOT by activity
    assert_eq!(
        result.items[0].title, "A Parent",
        "Should be first alphabetically despite older activity"
    );
    assert_eq!(
        result.items[1].title, "Z Parent",
        "Should be last alphabetically despite recent activity"
    );
}

// =============================================================================
// Subnote Count Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_parent_notes_include_subnote_count() {
    let db = setup_db().await;

    // Create parent note with 3 subnotes
    let parent_with_subnotes = Note {
        id: generate_id(),
        title: "Parent with children".to_string(),
        content: "Has subnotes".to_string(),
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
    db.notes().create(&parent_with_subnotes).await.unwrap();

    // Create 3 subnotes
    for i in 1..=3 {
        let subnote = Note {
            id: generate_id(),
            title: format!("Subnote {}", i),
            content: format!("Content {}", i),
            tags: vec![],
            parent_id: Some(parent_with_subnotes.id.clone()),
            idx: Some(i),
            repo_ids: vec![],
            project_ids: vec![],
            subnote_count: None,
            created_at: None,
            updated_at: None,
            deleted_at: None,
        };
        db.notes().create(&subnote).await.unwrap();
    }

    // Create parent note without subnotes
    let parent_no_subnotes = Note {
        id: generate_id(),
        title: "Parent alone".to_string(),
        content: "No subnotes".to_string(),
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
    db.notes().create(&parent_no_subnotes).await.unwrap();

    // Query parent notes with type=note
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };

    let result = db.notes().list(Some(&query)).await.unwrap();
    assert_eq!(result.total, 2, "Should return 2 parent notes");

    // Find each parent in results
    let parent_with = result
        .items
        .iter()
        .find(|n| n.id == parent_with_subnotes.id)
        .expect("Parent with subnotes should be in results");

    let parent_without = result
        .items
        .iter()
        .find(|n| n.id == parent_no_subnotes.id)
        .expect("Parent without subnotes should be in results");

    // Verify subnote_count is populated correctly
    assert_eq!(
        parent_with.subnote_count,
        Some(3),
        "Parent with 3 subnotes should have subnote_count = 3"
    );
    assert_eq!(
        parent_without.subnote_count,
        Some(0),
        "Parent with no subnotes should have subnote_count = 0"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_subnote_count_only_for_parent_notes() {
    let db = setup_db().await;

    // Create parent note
    let parent = Note {
        id: generate_id(),
        title: "Parent".to_string(),
        content: "Content".to_string(),
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
    db.notes().create(&parent).await.unwrap();

    // Create subnote
    let subnote = Note {
        id: generate_id(),
        title: "Subnote".to_string(),
        content: "Content".to_string(),
        tags: vec![],
        parent_id: Some(parent.id.clone()),
        idx: Some(1),
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    db.notes().create(&subnote).await.unwrap();

    // Query WITHOUT type filter - subnote_count should be None
    let result = db.notes().list(None).await.unwrap();
    for note in &result.items {
        assert_eq!(
            note.subnote_count, None,
            "subnote_count should be None when not filtering by type=note"
        );
    }

    // Query WITH type=note - subnote_count should be Some
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };
    let result = db.notes().list(Some(&query)).await.unwrap();
    let parent_result = result.items.iter().find(|n| n.id == parent.id).unwrap();
    assert_eq!(
        parent_result.subnote_count,
        Some(1),
        "subnote_count should be Some(1) when filtering by type=note"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_metadata_only_includes_subnote_count() {
    let db = setup_db().await;

    // Create parent with 2 subnotes
    let parent = Note {
        id: generate_id(),
        title: "Parent".to_string(),
        content: "This is the full content that should NOT be in metadata_only".to_string(),
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
    db.notes().create(&parent).await.unwrap();

    for i in 1..=2 {
        let subnote = Note {
            id: generate_id(),
            title: format!("Subnote {}", i),
            content: format!("Content {}", i),
            tags: vec![],
            parent_id: Some(parent.id.clone()),
            idx: Some(i),
            repo_ids: vec![],
            project_ids: vec![],
            subnote_count: None,
            created_at: None,
            updated_at: None,
            deleted_at: None,
        };
        db.notes().create(&subnote).await.unwrap();
    }

    // Query with list_metadata_only and type=note
    let query = NoteQuery {
        note_type: Some("note".to_string()),
        ..Default::default()
    };
    let result = db.notes().list_metadata_only(Some(&query)).await.unwrap();

    let parent_result = result.items.iter().find(|n| n.id == parent.id).unwrap();

    // Verify content is empty (metadata only)
    assert_eq!(
        parent_result.content, "",
        "Content should be empty in metadata_only"
    );

    // Verify subnote_count is still populated
    assert_eq!(
        parent_result.subnote_count,
        Some(2),
        "subnote_count should be Some(2) even in metadata_only"
    );
}
