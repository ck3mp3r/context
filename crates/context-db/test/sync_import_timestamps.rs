//! Tests for SyncRepository import timestamp preservation.

use crate::SqliteDatabase;
use context_core::{
    Database, HasNotes, HasProjects, HasSync, HasTaskLists, Note, NoteRepository, Project,
    ProjectRepository, SyncRepository, TaskList, TaskListRepository, TaskListStatus, write_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_preserves_timestamps() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a note with specific timestamps
    let original_created = "2024-01-01T10:00:00Z";
    let original_updated = "2024-01-02T15:30:00Z";

    let note = Note {
        id: "testid01".to_string(),
        title: "Test Note".to_string(),
        content: "Original content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some(original_created.to_string()),
        updated_at: Some(original_updated.to_string()),
        deleted_at: None,
    };

    db.notes().create(&note).await.unwrap();

    // Verify timestamps were stored correctly
    let stored_note = db.notes().get("testid01").await.unwrap();
    assert_eq!(stored_note.created_at, Some(original_created.to_string()));
    assert_eq!(stored_note.updated_at, Some(original_updated.to_string()));

    // Export the note
    db.sync().export_all(temp_dir.path()).await.unwrap();

    // Modify the note content and update timestamp
    let modified_updated = "2024-01-03T20:45:00Z";
    let modified_note = Note {
        id: "testid01".to_string(),
        title: "Test Note".to_string(),
        content: "Modified content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some(original_created.to_string()),
        updated_at: Some(modified_updated.to_string()),
        deleted_at: None,
    };

    // Write the modified note to JSONL (simulating external modification)
    write_jsonl(&temp_dir.path().join("notes.jsonl"), &[modified_note]).unwrap();

    // Import should preserve the modified timestamp
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify timestamps were preserved (not overwritten by trigger)
    let imported_note = db.notes().get("testid01").await.unwrap();
    assert_eq!(
        imported_note.created_at,
        Some(original_created.to_string()),
        "created_at should be preserved"
    );
    assert_eq!(
        imported_note.updated_at,
        Some(modified_updated.to_string()),
        "updated_at should be preserved from import, not auto-generated"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_preserves_project_timestamps() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project with specific timestamps
    let original_created = "2024-01-01T10:00:00Z";
    let original_updated = "2024-01-02T15:30:00Z";

    let project = Project {
        id: "proj1234".to_string(),
        title: "Test Project".to_string(),
        description: Some("Test description".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some(original_created.to_string()),
        updated_at: Some(original_updated.to_string()),
        deleted_at: None,
    };

    db.projects().create(&project).await.unwrap();

    // Modify and reimport with new timestamp
    let modified_updated = "2024-01-03T20:45:00Z";
    let modified_project = Project {
        id: "proj1234".to_string(),
        title: "Modified Project".to_string(),
        description: Some("Modified description".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some(original_created.to_string()),
        updated_at: Some(modified_updated.to_string()),
        deleted_at: None,
    };

    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[modified_project]).unwrap();
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let imported_project = db.projects().get("proj1234").await.unwrap();
    assert_eq!(
        imported_project.created_at.as_ref().unwrap(),
        original_created,
        "project created_at should be preserved"
    );
    assert_eq!(
        imported_project.updated_at.as_ref().unwrap(),
        modified_updated,
        "project updated_at should be preserved from import, not auto-generated"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_preserves_task_list_timestamps() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project first (required for task list)
    let project = Project {
        id: "proj1234".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db.projects().create(&project).await.unwrap();

    // Create a task list with specific timestamps
    let original_created = "2024-01-01T10:00:00Z";
    let original_updated = "2024-01-02T15:30:00Z";

    let task_list = TaskList {
        id: "list1234".to_string(),
        title: "Test List".to_string(),
        description: Some("Test description".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: "proj1234".to_string(),
        created_at: Some(original_created.to_string()),
        updated_at: Some(original_updated.to_string()),
        archived_at: None,
        deleted_at: None,
    };

    db.task_lists().create(&task_list).await.unwrap();

    // Modify and reimport with new timestamp
    let modified_updated = "2024-01-03T20:45:00Z";
    let modified_list = TaskList {
        id: "list1234".to_string(),
        title: "Modified List".to_string(),
        description: Some("Modified description".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: "proj1234".to_string(),
        created_at: Some(original_created.to_string()),
        updated_at: Some(modified_updated.to_string()),
        archived_at: None,
        deleted_at: None,
    };

    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();
    write_jsonl(&temp_dir.path().join("lists.jsonl"), &[modified_list]).unwrap();
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let imported_list = db.task_lists().get("list1234").await.unwrap();
    assert_eq!(
        imported_list.created_at.as_ref().unwrap(),
        original_created,
        "task_list created_at should be preserved"
    );
    assert_eq!(
        imported_list.updated_at.as_ref().unwrap(),
        modified_updated,
        "task_list updated_at should be preserved from import, not auto-generated"
    );
}
