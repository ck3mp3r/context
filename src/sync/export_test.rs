use crate::db::{
    Database, Note, NoteRepository, Project, ProjectRepository, Repo, RepoRepository,
    SqliteDatabase,
};
use crate::sync::export::*;
use crate::sync::jsonl::read_jsonl;
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_empty_database() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let summary = export_all(&db, temp_dir.path()).await.unwrap();

    // No default project in migrations
    assert_eq!(summary.repos, 0);
    assert_eq!(summary.projects, 0);
    assert_eq!(summary.task_lists, 0);
    assert_eq!(summary.tasks, 0);
    assert_eq!(summary.notes, 0);

    // Verify files exist
    assert!(temp_dir.path().join("repos.jsonl").exists());
    assert!(temp_dir.path().join("projects.jsonl").exists());
    assert!(temp_dir.path().join("lists.jsonl").exists());
    assert!(temp_dir.path().join("tasks.jsonl").exists());
    assert!(temp_dir.path().join("notes.jsonl").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_with_data() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create test data
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec!["test".to_string()],
        project_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.repos().create(&repo).await.unwrap();

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
    db.projects().create(&project).await.unwrap();

    // Export
    let summary = export_all(&db, temp_dir.path()).await.unwrap();

    assert_eq!(summary.repos, 1);
    assert_eq!(summary.projects, 1); // Just the one we created
    assert_eq!(summary.total(), 2);

    // Verify JSONL content
    let repos: Vec<Repo> = read_jsonl(&temp_dir.path().join("repos.jsonl")).unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].id, "12345678");

    let projects: Vec<Project> = read_jsonl(&temp_dir.path().join("projects.jsonl")).unwrap();
    assert_eq!(projects.len(), 1); // Just our test project

    // Get our test project
    let our_project = &projects[0];
    assert_eq!(our_project.title, "Test Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_creates_all_files() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    export_all(&db, temp_dir.path()).await.unwrap();

    // All 5 files should exist
    let expected_files = [
        "repos.jsonl",
        "projects.jsonl",
        "lists.jsonl",
        "tasks.jsonl",
        "notes.jsonl",
    ];

    for file in &expected_files {
        assert!(
            temp_dir.path().join(file).exists(),
            "File {} should exist",
            file
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_includes_relationships() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project first
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test project".to_string()),
        tags: vec![],
        external_ref: None,
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    // Create a repo linked to the project
    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec!["test".to_string()],
        project_ids: vec!["proj0001".to_string()],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.repos().create(&repo).await.unwrap();

    // Create a note with relationships
    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec!["repo0001".to_string()],
        project_ids: vec!["proj0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    // Export
    export_all(&db, temp_dir.path()).await.unwrap();

    // Read back the exported note
    let notes: Vec<Note> = read_jsonl(&temp_dir.path().join("notes.jsonl")).unwrap();
    let exported_note = notes.iter().find(|n| n.id == "note0001").unwrap();

    // Verify relationships are exported
    assert_eq!(exported_note.repo_ids, vec!["repo0001"]);
    assert_eq!(exported_note.project_ids, vec!["proj0001"]);

    // Read back the exported project
    let projects: Vec<Project> = read_jsonl(&temp_dir.path().join("projects.jsonl")).unwrap();
    let exported_project = projects.iter().find(|p| p.id == "proj0001").unwrap();

    // Verify project relationships are exported
    assert_eq!(exported_project.note_ids, vec!["note0001"]);
    assert_eq!(exported_project.repo_ids, vec!["repo0001"]);

    // Read back the exported repo
    let repos: Vec<Repo> = read_jsonl(&temp_dir.path().join("repos.jsonl")).unwrap();
    let exported_repo = repos.iter().find(|r| r.id == "repo0001").unwrap();

    // Verify repo relationships are exported
    assert_eq!(exported_repo.project_ids, vec!["proj0001"]);
}
