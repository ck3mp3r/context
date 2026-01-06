use crate::db::{
    Database, Note, NoteRepository, NoteType, Project, ProjectRepository, Repo, RepoRepository,
    SqliteDatabase,
};
use crate::sync::export::export_all;
use crate::sync::import::*;
use crate::sync::jsonl::write_jsonl;
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_empty_directory() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create empty JSONL files
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("projects.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    let summary = import_all(&db, temp_dir.path()).await.unwrap();

    assert_eq!(summary.repos, 0);
    assert_eq!(summary.projects, 0);
    assert_eq!(summary.task_lists, 0);
    assert_eq!(summary.tasks, 0);
    assert_eq!(summary.notes, 0);
    assert_eq!(summary.total(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_missing_files() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Don't create any files - should handle gracefully
    let summary = import_all(&db, temp_dir.path()).await.unwrap();

    assert_eq!(summary.total(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_then_import() {
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create test data in db1
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec!["test".to_string()],
        project_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db1.repos().create(&repo).await.unwrap();

    let project = Project {
        id: "abcdef12".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test".to_string()),
        tags: vec![],
        external_ref: None,
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db1.projects().create(&project).await.unwrap();

    // Export from db1
    let export_summary = export_all(&db1, temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.repos, 1);
    assert_eq!(export_summary.projects, 1); // Just test project

    // Import to db2
    let import_summary = import_all(&db2, temp_dir.path()).await.unwrap();
    assert_eq!(import_summary.repos, 1);
    assert_eq!(import_summary.projects, 1); // Just test project

    // Verify data in db2
    let repos = db2.repos().list(None).await.unwrap();
    assert_eq!(repos.items.len(), 1);
    assert_eq!(repos.items[0].id, "12345678");

    let projects = db2.projects().list(None).await.unwrap();
    // db2 has just the imported project
    assert_eq!(projects.items.len(), 1);

    // Get our imported project
    let imported_project = &projects.items[0];
    assert_eq!(imported_project.title, "Test Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_updates_existing() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create initial repo
    let repo_v1 = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo1".to_string(),
        path: Some("/test/path1".to_string()),
        tags: vec!["v1".to_string()],
        project_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.repos().create(&repo_v1).await.unwrap();

    // Create modified version in JSONL
    let repo_v2 = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo2".to_string(), // Changed
        path: Some("/test/path2".to_string()),               // Changed
        tags: vec!["v2".to_string()],                        // Changed
        project_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };

    write_jsonl(&temp_dir.path().join("repos.jsonl"), &[repo_v2]).unwrap();

    // Create empty files for other entities
    std::fs::write(temp_dir.path().join("projects.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    // Import should update the existing repo
    let summary = import_all(&db, temp_dir.path()).await.unwrap();
    assert_eq!(summary.repos, 1);

    // Verify it was updated, not duplicated
    let repos = db.repos().list(None).await.unwrap();
    assert_eq!(repos.items.len(), 1);

    let updated_repo = db.repos().get("12345678").await.unwrap();
    assert_eq!(updated_repo.remote, "https://github.com/test/repo2");
    assert_eq!(updated_repo.path, Some("/test/path2".to_string()));
    assert_eq!(updated_repo.tags, vec!["v2".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_preserves_relationships() {
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create entities with relationships in db1
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_ref: None,
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db1.projects().create(&project).await.unwrap();

    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec!["proj0001".to_string()],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db1.repos().create(&repo).await.unwrap();

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        note_type: NoteType::Manual,
        parent_id: None,
        idx: None,
        repo_ids: vec!["repo0001".to_string()],
        project_ids: vec!["proj0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
    };
    db1.notes().create(&note).await.unwrap();

    // Export from db1
    export_all(&db1, temp_dir.path()).await.unwrap();

    // Import to db2
    import_all(&db2, temp_dir.path()).await.unwrap();

    // Verify relationships are preserved
    let imported_project = db2.projects().get("proj0001").await.unwrap();
    assert_eq!(imported_project.repo_ids, vec!["repo0001"]);
    assert_eq!(imported_project.note_ids, vec!["note0001"]);

    let imported_repo = db2.repos().get("repo0001").await.unwrap();
    assert_eq!(imported_repo.project_ids, vec!["proj0001"]);

    let imported_note = db2.notes().get("note0001").await.unwrap();
    assert_eq!(imported_note.project_ids, vec!["proj0001"]);
    assert_eq!(imported_note.repo_ids, vec!["repo0001"]);
}
