use crate::SqliteDatabase;
use crate::helpers::{read_deleted_at, read_updated_at};
use context_core::{Database, HasNotes, Note, NoteRepository};

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
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_note_clears_deleted_at() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note("noterstr", "Restore Me", "Restorable content"))
        .await
        .expect("Create should succeed");
    notes
        .delete("noterstr")
        .await
        .expect("Delete should succeed");

    notes
        .restore("noterstr")
        .await
        .expect("Restore should succeed");

    assert!(
        read_deleted_at(db.pool(), "note", "noterstr")
            .await
            .is_none(),
        "restore should clear deleted_at"
    );
    assert_ne!(
        read_updated_at(db.pool(), "note", "noterstr").await,
        Some("2025-01-01 00:00:00".to_string()),
        "restore should bump updated_at"
    );
    notes
        .get("noterstr")
        .await
        .expect("restored note should be readable");
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_note_restores_fts_entry() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note(
            "notefts1",
            "Orchestration",
            "kubernetes cluster bootstrap",
        ))
        .await
        .expect("Create should succeed");

    notes
        .delete("notefts1")
        .await
        .expect("Delete should succeed");
    let deleted_results = notes
        .search("kubernetes", None)
        .await
        .expect("Search should succeed");
    assert!(
        deleted_results.items.is_empty(),
        "a soft-deleted note must not be searchable"
    );

    notes
        .restore("notefts1")
        .await
        .expect("Restore should succeed");

    let restored_results = notes
        .search("kubernetes", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        restored_results.items.len(),
        1,
        "restore should re-index the note in note_fts"
    );
    assert_eq!(restored_results.items[0].id, "notefts1");
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_nonexistent_note_returns_not_found() {
    let db = setup_db().await;

    let result = db.notes().restore("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "restoring a missing note should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_live_note_is_noop() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note("notelive", "Live Note", "Never deleted"))
        .await
        .expect("Create should succeed");

    notes
        .restore("notelive")
        .await
        .expect("Restoring a live note should be a no-op");

    assert!(
        read_deleted_at(db.pool(), "note", "notelive")
            .await
            .is_none(),
        "deleted_at should stay NULL"
    );
    assert_eq!(
        read_updated_at(db.pool(), "note", "notelive").await,
        Some("2025-01-01 00:00:00".to_string()),
        "a no-op restore must not bump updated_at"
    );
}
