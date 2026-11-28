//! Integration tests for note archive state: status, archived_at and sync.

use crate::SqliteDatabase;
use context_core::{
    Database, HasNotes, HasSync, Note, NoteQuery, NoteRepository, NoteStatus, SyncRepository,
    generate_entity_id,
};
use tempfile::TempDir;

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

fn make_note(title: &str) -> Note {
    Note {
        id: generate_entity_id(),
        title: title.to_string(),
        content: format!("{title} content"),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        status: NoteStatus::Active,
        created_at: None,
        updated_at: None,
        archived_at: None,
        deleted_at: None,
    }
}

fn status_query(status: Option<&str>) -> NoteQuery {
    NoteQuery {
        status: status.map(str::to_string),
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn note_create_defaults_to_active_without_archived_at() {
    let db = setup_db().await;
    let notes = db.notes();

    let created = notes
        .create(&make_note("Active note"))
        .await
        .expect("Create should succeed");
    assert_eq!(created.status, NoteStatus::Active);
    assert_eq!(created.archived_at, None);

    let fetched = notes.get(&created.id).await.expect("Get should succeed");
    assert_eq!(fetched.status, NoteStatus::Active);
    assert_eq!(fetched.archived_at, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_archive_sets_archived_at() {
    let db = setup_db().await;
    let notes = db.notes();

    let created = notes
        .create(&make_note("Archive me"))
        .await
        .expect("Create should succeed");

    let mut archived = created.clone();
    archived.status = NoteStatus::Archived;
    notes
        .update(&archived)
        .await
        .expect("Update should succeed");

    let after = notes.get(&created.id).await.expect("Get should succeed");
    assert_eq!(after.status, NoteStatus::Archived);
    assert!(
        after.archived_at.is_some(),
        "archived_at must be set when status changes to archived"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_archive_twice_is_idempotent() {
    let db = setup_db().await;
    let notes = db.notes();

    let created = notes
        .create(&make_note("Idempotent archive"))
        .await
        .expect("Create should succeed");

    let mut first = created.clone();
    first.status = NoteStatus::Archived;
    notes.update(&first).await.expect("Update should succeed");

    let after_first = notes.get(&created.id).await.expect("Get should succeed");
    let first_archived_at = after_first.archived_at.clone();
    assert!(first_archived_at.is_some());

    // Repeat the archive without supplying archived_at.
    let mut second = after_first.clone();
    second.status = NoteStatus::Archived;
    second.archived_at = None;
    notes.update(&second).await.expect("Update should succeed");

    let after_second = notes.get(&created.id).await.expect("Get should succeed");
    assert_eq!(
        after_second.archived_at, first_archived_at,
        "repeating the archive must not change archived_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_unarchive_clears_archived_at() {
    let db = setup_db().await;
    let notes = db.notes();

    let mut note = make_note("Archived note");
    note.status = NoteStatus::Archived;
    note.archived_at = Some("2025-01-01 12:00:00".to_string());
    let created = notes.create(&note).await.expect("Create should succeed");
    assert_eq!(created.status, NoteStatus::Archived);
    assert!(created.archived_at.is_some());

    let mut unarchived = created.clone();
    unarchived.status = NoteStatus::Active;
    notes
        .update(&unarchived)
        .await
        .expect("Update should succeed");

    let after = notes.get(&created.id).await.expect("Get should succeed");
    assert_eq!(after.status, NoteStatus::Active);
    assert_eq!(
        after.archived_at, None,
        "unarchiving must clear archived_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_update_with_unchanged_status_keeps_archived_at() {
    let db = setup_db().await;
    let notes = db.notes();

    let mut note = make_note("Still archived");
    note.status = NoteStatus::Archived;
    note.archived_at = Some("2025-01-01 12:00:00".to_string());
    let created = notes.create(&note).await.expect("Create should succeed");

    let mut renamed = created.clone();
    renamed.title = "Still archived, renamed".to_string();
    renamed.archived_at = None;
    notes.update(&renamed).await.expect("Update should succeed");

    let after = notes.get(&created.id).await.expect("Get should succeed");
    assert_eq!(after.status, NoteStatus::Archived);
    assert_eq!(
        after.archived_at, created.archived_at,
        "an unchanged status must keep the prior archived_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_list_filters_by_status() {
    let db = setup_db().await;
    let notes = db.notes();

    let active = notes
        .create(&make_note("Live active"))
        .await
        .expect("Create should succeed");

    let mut archived_input = make_note("Live archived");
    archived_input.status = NoteStatus::Archived;
    archived_input.archived_at = Some("2025-01-01 12:00:00".to_string());
    let archived = notes
        .create(&archived_input)
        .await
        .expect("Create should succeed");

    let archived_only = notes
        .list(Some(&status_query(Some("archived"))))
        .await
        .expect("List should succeed");
    assert_eq!(archived_only.total, 1);
    assert_eq!(archived_only.items[0].id, archived.id);
    assert_eq!(archived_only.items[0].status, NoteStatus::Archived);
    assert_eq!(archived_only.items[0].archived_at, archived.archived_at);

    let active_only = notes
        .list(Some(&status_query(Some("active"))))
        .await
        .expect("List should succeed");
    assert_eq!(active_only.total, 1);
    assert_eq!(active_only.items[0].id, active.id);
    assert_eq!(active_only.items[0].status, NoteStatus::Active);
    assert_eq!(active_only.items[0].archived_at, None);

    let all = notes
        .list(Some(&status_query(None)))
        .await
        .expect("List should succeed");
    assert_eq!(all.total, 2, "an omitted status must return both statuses");
}

#[tokio::test(flavor = "multi_thread")]
async fn note_list_metadata_only_filters_by_status() {
    let db = setup_db().await;
    let notes = db.notes();

    notes
        .create(&make_note("Meta active"))
        .await
        .expect("Create should succeed");

    let mut archived_input = make_note("Meta archived");
    archived_input.status = NoteStatus::Archived;
    archived_input.archived_at = Some("2025-01-01 12:00:00".to_string());
    let archived = notes
        .create(&archived_input)
        .await
        .expect("Create should succeed");

    let archived_only = notes
        .list_metadata_only(Some(&status_query(Some("archived"))))
        .await
        .expect("List metadata should succeed");
    assert_eq!(archived_only.total, 1);
    assert_eq!(archived_only.items[0].id, archived.id);
    assert_eq!(archived_only.items[0].status, NoteStatus::Archived);
    assert_eq!(archived_only.items[0].archived_at, archived.archived_at);

    let active_only = notes
        .list_metadata_only(Some(&status_query(Some("active"))))
        .await
        .expect("List metadata should succeed");
    assert_eq!(active_only.total, 1);
    assert_eq!(active_only.items[0].status, NoteStatus::Active);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_search_filters_by_status() {
    let db = setup_db().await;
    let notes = db.notes();

    let mut active_input = make_note("Searchable active");
    active_input.content = "archive keyword in active note".to_string();
    notes
        .create(&active_input)
        .await
        .expect("Create should succeed");

    let mut archived_input = make_note("Searchable archived");
    archived_input.content = "archive keyword in archived note".to_string();
    archived_input.status = NoteStatus::Archived;
    archived_input.archived_at = Some("2025-01-01 12:00:00".to_string());
    let archived = notes
        .create(&archived_input)
        .await
        .expect("Create should succeed");

    let archived_only = notes
        .search("archive", Some(&status_query(Some("archived"))))
        .await
        .expect("Search should succeed");
    assert_eq!(archived_only.total, 1);
    assert_eq!(archived_only.items[0].id, archived.id);
    assert_eq!(archived_only.items[0].status, NoteStatus::Archived);
    assert_eq!(archived_only.items[0].archived_at, archived.archived_at);

    let active_only = notes
        .search("archive", Some(&status_query(Some("active"))))
        .await
        .expect("Search should succeed");
    assert_eq!(active_only.total, 1);
    assert_eq!(active_only.items[0].status, NoteStatus::Active);

    let all = notes
        .search("archive", Some(&status_query(None)))
        .await
        .expect("Search should succeed");
    assert_eq!(all.total, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_status_filter_still_excludes_tombstones() {
    let db = setup_db().await;
    let notes = db.notes();

    let mut archived_input = make_note("Deleted archived");
    archived_input.content = "archive keyword in deleted note".to_string();
    archived_input.status = NoteStatus::Archived;
    archived_input.archived_at = Some("2025-01-01 12:00:00".to_string());
    let archived = notes
        .create(&archived_input)
        .await
        .expect("Create should succeed");

    notes
        .delete(&archived.id)
        .await
        .expect("Delete should succeed");

    let listed = notes
        .list(Some(&status_query(Some("archived"))))
        .await
        .expect("List should succeed");
    assert_eq!(listed.total, 0, "tombstones must not appear in list");

    let searched = notes
        .search("archive", Some(&status_query(Some("archived"))))
        .await
        .expect("Search should succeed");
    assert_eq!(searched.total, 0, "tombstones must not appear in search");
}

#[tokio::test(flavor = "multi_thread")]
async fn note_restore_keeps_status_and_archived_at() {
    let db = setup_db().await;
    let notes = db.notes();

    let mut note = make_note("Archived then deleted");
    note.status = NoteStatus::Archived;
    note.archived_at = Some("2025-01-01 12:00:00".to_string());
    let created = notes.create(&note).await.expect("Create should succeed");

    notes
        .delete(&created.id)
        .await
        .expect("Delete should succeed");
    notes
        .restore(&created.id)
        .await
        .expect("Restore should succeed");

    let restored = notes.get(&created.id).await.expect("Get should succeed");
    assert_eq!(restored.status, NoteStatus::Archived);
    assert_eq!(restored.archived_at, created.archived_at);
}

#[tokio::test(flavor = "multi_thread")]
async fn note_archive_state_roundtrips_through_export_import() {
    let db1 = setup_db().await;
    let db2 = setup_db().await;
    let temp_dir = TempDir::new().unwrap();

    let mut note = make_note("Synced archive");
    note.created_at = Some("2024-01-01 00:00:00".to_string());
    note.updated_at = Some("2024-01-01 00:00:00".to_string());
    let created = db1
        .notes()
        .create(&note)
        .await
        .expect("Create should succeed");

    // First export/import: the note is active.
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    let imported_active = db2.notes().get(&created.id).await.unwrap();
    assert_eq!(imported_active.status, NoteStatus::Active);
    assert_eq!(imported_active.archived_at, None);

    // Archive it on the source and export the newer replica.
    let mut archived = imported_active.clone();
    archived.status = NoteStatus::Archived;
    archived.updated_at = Some("2024-02-01 00:00:00".to_string());
    db1.notes()
        .update(&archived)
        .await
        .expect("Update should succeed");

    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    let imported_archived = db2.notes().get(&created.id).await.unwrap();
    assert_eq!(imported_archived.status, NoteStatus::Archived);
    assert!(
        imported_archived.archived_at.is_some(),
        "archived_at must survive export/import"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn note_import_without_status_defaults_to_active() {
    let db = setup_db().await;
    let temp_dir = TempDir::new().unwrap();

    // A notes.jsonl written before the archive columns existed.
    let line = r#"{"id":"noteold1","title":"Legacy note","content":"Legacy content","tags":[],"parent_id":null,"idx":null,"repo_ids":[],"project_ids":[],"created_at":"2024-01-01 00:00:00","updated_at":"2024-01-01 00:00:00","deleted_at":null}"#;
    std::fs::write(temp_dir.path().join("notes.jsonl"), format!("{line}\n"))
        .expect("notes.jsonl should be written");

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let imported = db.notes().get("noteold1").await.unwrap();
    assert_eq!(imported.status, NoteStatus::Active);
    assert_eq!(imported.archived_at, None);
}
