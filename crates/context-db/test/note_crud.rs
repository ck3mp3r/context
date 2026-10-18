//! Tests for SqliteNoteRepository.

use crate::SqliteDatabase;
use crate::helpers::{count_rows, read_deleted_at, read_updated_at};
use context_core::{Database, HasNotes, Note, NoteQuery, NoteRepository};

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
async fn note_create_and_get() {
    let db = setup_db().await;
    let notes = db.notes();

    let note = Note {
        id: "note0001".to_string(),
        title: "My First Note".to_string(),
        content: "This is markdown content\n\n## Heading\n\nWith paragraphs.".to_string(),
        tags: vec!["session".to_string(), "important".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec![],    // Empty by default - relationships managed separately
        project_ids: vec![], // Empty by default - relationships managed separately
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };

    notes.create(&note).await.expect("Create should succeed");

    let retrieved = notes.get("note0001").await.expect("Get should succeed");
    assert_eq!(retrieved.id, note.id);
    assert_eq!(retrieved.title, note.title);
    assert_eq!(retrieved.content, note.content);
    assert_eq!(retrieved.tags, note.tags);
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
    notes.update(&note).await.expect("Update should succeed");

    let retrieved = notes.get("noteupd1").await.expect("Get should succeed");
    assert_eq!(retrieved.title, "Updated Title");
    assert_eq!(retrieved.content, "Updated content with more text");
    assert_eq!(retrieved.tags, vec!["updated".to_string()]);
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

    let deleted_at = read_deleted_at(db.pool(), "note", "notedel1").await;
    assert!(deleted_at.is_some(), "note should be soft-deleted");

    let updated_at = read_updated_at(db.pool(), "note", "notedel1").await;
    assert_ne!(
        updated_at,
        Some("2025-01-01 00:00:00".to_string()),
        "updated_at should be bumped by the soft-delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_delete_cascades_to_relationships_and_orphans_children() {
    let db = setup_db().await;
    let notes = db.notes();

    sqlx::query(
        "INSERT INTO project (id, title, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("projnote")
    .bind("Note Project")
    .bind("[]")
    .bind("2025-01-01 00:00:00")
    .bind("2025-01-01 00:00:00")
    .execute(db.pool())
    .await
    .unwrap();

    sqlx::query("INSERT INTO repo (id, remote, tags, created_at) VALUES (?, ?, ?, ?)")
        .bind("reponote")
        .bind("github:note/test")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .unwrap();

    let mut parent = make_note("notepar1", "Parent", "Parent content");
    parent.project_ids = vec!["projnote".to_string()];
    parent.repo_ids = vec!["reponote".to_string()];
    notes.create(&parent).await.expect("Create parent note");

    let mut child = make_note("notechi1", "Child", "Child content");
    child.parent_id = Some("notepar1".to_string());
    notes.create(&child).await.expect("Create child note");

    notes.delete("notepar1").await.expect("Delete parent note");

    assert!(
        read_deleted_at(db.pool(), "note", "notepar1")
            .await
            .is_some(),
        "parent note should be soft-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "project_note", "note_id = ?", "notepar1").await,
        0,
        "project_note rows should be hard-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "note_repo", "note_id = ?", "notepar1").await,
        0,
        "note_repo rows should be hard-deleted"
    );

    let child_parent: Option<String> =
        sqlx::query_scalar("SELECT parent_id FROM note WHERE id = ?")
            .bind("notechi1")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(
        child_parent.is_none(),
        "child note should be orphaned (parent_id = NULL)"
    );
    assert!(
        read_deleted_at(db.pool(), "note", "notechi1")
            .await
            .is_none(),
        "child note should stay live"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_already_deleted_is_noop() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note("notenoop", "Delete Twice", "Content"))
        .await
        .expect("Create note");

    notes.delete("notenoop").await.unwrap();
    let first_deleted_at = read_deleted_at(db.pool(), "note", "notenoop").await;
    let first_updated_at = read_updated_at(db.pool(), "note", "notenoop").await;

    notes
        .delete("notenoop")
        .await
        .expect("Second delete should be a no-op");

    assert_eq!(
        read_deleted_at(db.pool(), "note", "notenoop").await,
        first_deleted_at,
        "deleted_at should not change on a repeated delete"
    );
    assert_eq!(
        read_updated_at(db.pool(), "note", "notenoop").await,
        first_updated_at,
        "updated_at should not be bumped on a repeated delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_nonexistent_note_returns_not_found() {
    let db = setup_db().await;

    let result = db.notes().delete("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "deleting a missing note should return NotFound, got: {:?}",
        result
    );
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
async fn list_metadata_only_filters_by_project_id() {
    let db = setup_db().await;
    let pool = db.pool();

    // Create two projects directly
    sqlx::query("INSERT INTO project (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
        .bind("proj0001")
        .bind("Project One")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(pool)
        .await
        .expect("Insert project 1");

    sqlx::query("INSERT INTO project (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
        .bind("proj0002")
        .bind("Project Two")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(pool)
        .await
        .expect("Insert project 2");

    // Create three notes
    let notes = db.notes();
    notes
        .create(&make_note("mnote001", "Note A", "content a"))
        .await
        .expect("Create note A");
    notes
        .create(&make_note("mnote002", "Note B", "content b"))
        .await
        .expect("Create note B");
    notes
        .create(&make_note("mnote003", "Note C", "content c"))
        .await
        .expect("Create note C");

    // Link: Note A -> project 1, Note B -> project 2, Note C -> no project
    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("proj0001")
        .bind("mnote001")
        .execute(pool)
        .await
        .expect("Link note A to project 1");

    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("proj0002")
        .bind("mnote002")
        .execute(pool)
        .await
        .expect("Link note B to project 2");

    // Filter by project 1 -> should return only Note A
    let q1 = NoteQuery {
        project_id: Some("proj0001".to_string()),
        ..Default::default()
    };
    let result1 = notes
        .list_metadata_only(Some(&q1))
        .await
        .expect("list_metadata_only with project 1");
    assert_eq!(result1.total, 1, "project 1 should have 1 note");
    assert_eq!(result1.items[0].id, "mnote001");

    // Filter by project 2 -> should return only Note B
    let q2 = NoteQuery {
        project_id: Some("proj0002".to_string()),
        ..Default::default()
    };
    let result2 = notes
        .list_metadata_only(Some(&q2))
        .await
        .expect("list_metadata_only with project 2");
    assert_eq!(result2.total, 1, "project 2 should have 1 note");
    assert_eq!(result2.items[0].id, "mnote002");

    // Filter by non-existent project -> should return 0
    let q3 = NoteQuery {
        project_id: Some("proj9999".to_string()),
        ..Default::default()
    };
    let result3 = notes
        .list_metadata_only(Some(&q3))
        .await
        .expect("list_metadata_only with non-existent project");
    assert_eq!(
        result3.total, 0,
        "non-existent project should return 0 notes"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_populates_project_ids() {
    let db = setup_db().await;
    let pool = db.pool();

    // Create a project
    sqlx::query("INSERT INTO project (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
        .bind("projlst1")
        .bind("Test Project")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(pool)
        .await
        .expect("Insert project");

    // Create two notes
    let notes = db.notes();
    notes
        .create(&make_note("listp001", "Note A", "content a"))
        .await
        .expect("Create note A");
    notes
        .create(&make_note("listp002", "Note B", "content b"))
        .await
        .expect("Create note B");

    // Link only Note A to the project
    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("projlst1")
        .bind("listp001")
        .execute(pool)
        .await
        .expect("Link note A to project");

    // List all notes
    let result = notes.list(None).await.expect("list should succeed");
    assert_eq!(result.total, 2, "should have 2 notes");

    // Note A should have project_ids populated
    let note_a = result.items.iter().find(|n| n.id == "listp001").unwrap();
    assert_eq!(
        note_a.project_ids,
        vec!["projlst1".to_string()],
        "Note A should have project_ids populated"
    );

    // Note B should have empty project_ids
    let note_b = result.items.iter().find(|n| n.id == "listp002").unwrap();
    assert!(
        note_b.project_ids.is_empty(),
        "Note B should have empty project_ids"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_metadata_only_populates_project_ids() {
    let db = setup_db().await;
    let pool = db.pool();

    // Create a project
    sqlx::query("INSERT INTO project (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
        .bind("promet01")
        .bind("Test Project")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(pool)
        .await
        .expect("Insert project");

    // Create two notes
    let notes = db.notes();
    notes
        .create(&make_note("metap001", "Note A", "content a"))
        .await
        .expect("Create note A");
    notes
        .create(&make_note("metap002", "Note B", "content b"))
        .await
        .expect("Create note B");

    // Link only Note A to the project
    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("promet01")
        .bind("metap001")
        .execute(pool)
        .await
        .expect("Link note A to project");

    // List metadata only
    let result = notes
        .list_metadata_only(None)
        .await
        .expect("list_metadata_only should succeed");
    assert_eq!(result.total, 2, "should have 2 notes");

    // Note A should have project_ids populated
    let note_a = result.items.iter().find(|n| n.id == "metap001").unwrap();
    assert_eq!(
        note_a.project_ids,
        vec!["promet01".to_string()],
        "Note A should have project_ids populated"
    );

    // Note B should have empty project_ids
    let note_b = result.items.iter().find(|n| n.id == "metap002").unwrap();
    assert!(
        note_b.project_ids.is_empty(),
        "Note B should have empty project_ids"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn search_populates_project_ids() {
    let db = setup_db().await;
    let pool = db.pool();

    // Create a project
    sqlx::query("INSERT INTO project (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
        .bind("prosrc01")
        .bind("Test Project")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(pool)
        .await
        .expect("Insert project");

    // Create two notes with searchable content
    let notes = db.notes();
    notes
        .create(&make_note(
            "srcp0001",
            "Rust Note",
            "rust programming content",
        ))
        .await
        .expect("Create note A");
    notes
        .create(&make_note(
            "srcp0002",
            "Python Note",
            "python programming content",
        ))
        .await
        .expect("Create note B");

    // Link only Note A to the project
    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("prosrc01")
        .bind("srcp0001")
        .execute(pool)
        .await
        .expect("Link note A to project");

    // Search for "programming" - should find both notes
    let result = notes
        .search("programming", None)
        .await
        .expect("search should succeed");
    assert_eq!(result.items.len(), 2, "should find 2 notes");

    // Note A should have project_ids populated
    let note_a = result.items.iter().find(|n| n.id == "srcp0001").unwrap();
    assert_eq!(
        note_a.project_ids,
        vec!["prosrc01".to_string()],
        "Note A should have project_ids populated"
    );

    // Note B should have empty project_ids
    let note_b = result.items.iter().find(|n| n.id == "srcp0002").unwrap();
    assert!(
        note_b.project_ids.is_empty(),
        "Note B should have empty project_ids"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_with_project_filter_populates_project_ids() {
    let db = setup_db().await;
    let pool = db.pool();

    // Create two projects
    sqlx::query("INSERT INTO project (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
        .bind("proflt01")
        .bind("Project One")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(pool)
        .await
        .expect("Insert project 1");

    sqlx::query("INSERT INTO project (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
        .bind("proflt02")
        .bind("Project Two")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(pool)
        .await
        .expect("Insert project 2");

    // Create two notes
    let notes = db.notes();
    notes
        .create(&make_note("fltpa001", "Note A", "content a"))
        .await
        .expect("Create note A");
    notes
        .create(&make_note("fltpa002", "Note B", "content b"))
        .await
        .expect("Create note B");

    // Link Note A to both projects, Note B to project 1 only
    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("proflt01")
        .bind("fltpa001")
        .execute(pool)
        .await
        .expect("Link note A to project 1");

    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("proflt02")
        .bind("fltpa001")
        .execute(pool)
        .await
        .expect("Link note A to project 2");

    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("proflt01")
        .bind("fltpa002")
        .execute(pool)
        .await
        .expect("Link note B to project 1");

    // Filter by project 1 -> should return both notes
    let q = NoteQuery {
        project_id: Some("proflt01".to_string()),
        ..Default::default()
    };
    let result = notes
        .list(Some(&q))
        .await
        .expect("list with project filter");
    assert_eq!(result.total, 2, "project 1 should have 2 notes");

    // Note A should have both project_ids populated
    let note_a = result.items.iter().find(|n| n.id == "fltpa001").unwrap();
    assert_eq!(
        note_a.project_ids.len(),
        2,
        "Note A should have 2 project_ids"
    );
    assert!(
        note_a.project_ids.contains(&"proflt01".to_string()),
        "Note A should contain proflt01"
    );
    assert!(
        note_a.project_ids.contains(&"proflt02".to_string()),
        "Note A should contain proflt02"
    );

    // Note B should have project 1 only
    let note_b = result.items.iter().find(|n| n.id == "fltpa002").unwrap();
    assert_eq!(
        note_b.project_ids,
        vec!["proflt01".to_string()],
        "Note B should have project 1 only"
    );
}
