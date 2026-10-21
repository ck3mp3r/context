//! Tests for SyncRepository export/import roundtrips.

use crate::SqliteDatabase;
use context_core::{
    Database, HasNotes, HasProjects, HasRepos, HasSync, HasTaskLists, HasTasks, Project,
    ProjectRepository, Repo, RepoRepository, SyncRepository, TaskList, TaskListRepository,
    TaskListStatus,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_then_import_roundtrip() {
    // Create DB with data
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test project".to_string()),
        tags: vec!["test".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project).await.unwrap();

    // Create a repo linked to the project
    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec!["git".to_string()],
        project_ids: vec!["proj0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    db1.repos().create(&repo).await.unwrap();

    // Export from db1 using sync repository
    let export_summary = db1.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.repos, 1, "Should export 1 repo");
    // Note: db1 has 1 default project + our test project = 2
    assert!(
        export_summary.projects >= 1,
        "Should export at least 1 project"
    );

    // Import to db2 using sync repository
    let import_summary = db2.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(import_summary.repos, 1, "Should import 1 repo");
    assert!(
        import_summary.projects >= 1,
        "Should import at least 1 project"
    );

    // Verify data in db2
    let imported_repo = db2.repos().get("repo0001").await.unwrap();
    assert_eq!(imported_repo.remote, "https://github.com/test/repo");
    assert_eq!(imported_repo.project_ids, vec!["proj0001"]);
    assert_eq!(imported_repo.tags, vec!["git"]);

    let imported_project = db2.projects().get("proj0001").await.unwrap();
    assert_eq!(imported_project.title, "Test Project");
    assert_eq!(imported_project.repo_ids, vec!["repo0001"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_export_preserves_all_relationships() {
    use context_core::{Note, NoteRepository, TaskList, TaskListRepository, TaskListStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create complex data with all relationship types
    let project = Project {
        id: "proj0001".to_string(),
        title: "Complex Project".to_string(),
        description: None,
        tags: vec!["complex".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project).await.unwrap();

    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec!["proj0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    db1.repos().create(&repo).await.unwrap();

    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Task List".to_string(),
        description: None,
        notes: None,
        project_id: "proj0001".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec!["repo0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db1.task_lists().create(&task_list).await.unwrap();

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Note content".to_string(),
        tags: vec!["important".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec!["repo0001".to_string()],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&note).await.unwrap();

    // Export and import
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify all relationships preserved
    let imported_project = db2.projects().get("proj0001").await.unwrap();
    assert_eq!(imported_project.repo_ids, vec!["repo0001"]);
    assert_eq!(imported_project.task_list_ids, vec!["list0001"]);
    assert_eq!(imported_project.note_ids, vec!["note0001"]);

    let imported_repo = db2.repos().get("repo0001").await.unwrap();
    assert_eq!(imported_repo.project_ids, vec!["proj0001"]);

    let imported_list = db2.task_lists().get("list0001").await.unwrap();
    assert_eq!(imported_list.project_id, "proj0001");
    assert_eq!(imported_list.repo_ids, vec!["repo0001"]);

    let imported_note = db2.notes().get("note0001").await.unwrap();
    assert_eq!(imported_note.project_ids, vec!["proj0001"]);
    assert_eq!(imported_note.repo_ids, vec!["repo0001"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_preserves_task_updated_at() {
    use context_core::{TaskRepository, TaskStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a test project
    let project = Project {
        id: "projtest".to_string(),
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
    db1.projects().create(&project).await.unwrap();
    let project_id = project.id.clone();

    // Create a task list
    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        project_id,
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db1.task_lists().create(&task_list).await.unwrap();

    // Create task with specific timestamps directly via SQL (bypassing create() which overwrites timestamps)
    sqlx::query(
        "INSERT INTO task (id, list_id, parent_id, title, description, status, priority, tags, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind("task0001")
    .bind("list0001")
    .bind(None::<String>)
    .bind("Test Task")
    .bind(Some("Description"))
    .bind("in_progress")
    .bind(Some(2))
    .bind(r#"["test"]"#)
    .bind("2024-01-01T10:00:00Z")
    .bind("2024-01-01T12:00:00Z") // CRITICAL: Must be preserved
    .execute(db1.pool())
    .await
    .unwrap();

    // Export and import
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify updated_at is preserved
    let imported_task = db2.tasks().get("task0001").await.unwrap();
    assert_eq!(
        imported_task.updated_at,
        Some("2024-01-01T12:00:00Z".to_string()),
        "CRITICAL: updated_at must be preserved during export/import!"
    );
    assert_eq!(
        imported_task.created_at,
        Some("2024-01-01T10:00:00Z".to_string())
    );
    assert_eq!(imported_task.title, "Test Task");
    assert_eq!(imported_task.status, TaskStatus::InProgress);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_preserves_repo_updated_at() {
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2024-01-01T10:00:00Z".to_string()),
        updated_at: Some("2024-01-01T12:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.repos().create(&repo).await.unwrap();

    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    let imported = db2.repos().get("repo0001").await.unwrap();
    assert_eq!(
        imported.updated_at,
        Some("2024-01-01T12:00:00Z".to_string()),
        "repo updated_at must survive export/import"
    );
    assert_eq!(
        imported.created_at,
        Some("2024-01-01T10:00:00Z".to_string())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_preserves_note_hierarchy() {
    // RED: This test will fail because parent_id and idx are not preserved during import
    use context_core::NoteRepository;

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project
    let project = Project {
        id: "proj0001".to_string(),
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
    db1.projects().create(&project).await.unwrap();

    // Create parent note
    let parent_note = context_core::Note {
        id: "note0001".to_string(),
        title: "Parent Note".to_string(),
        content: "This is the parent".to_string(),
        tags: vec!["parent".to_string()],
        parent_id: None,
        idx: Some(1),
        repo_ids: vec![],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&parent_note).await.unwrap();

    // Create child note with parent_id and idx
    let child_note = context_core::Note {
        id: "note0002".to_string(),
        title: "Child Note".to_string(),
        content: "This is a child".to_string(),
        tags: vec!["child".to_string()],
        parent_id: Some("note0001".to_string()), // CRITICAL: Must be preserved
        idx: Some(2),                            // CRITICAL: Must be preserved
        repo_ids: vec![],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-02T00:00:00Z".to_string()),
        updated_at: Some("2024-01-02T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&child_note).await.unwrap();

    // Create another child with different idx
    let child_note2 = context_core::Note {
        id: "note0003".to_string(),
        title: "Second Child".to_string(),
        content: "Another child".to_string(),
        tags: vec![],
        parent_id: Some("note0001".to_string()), // CRITICAL: Must be preserved
        idx: Some(1),                            // CRITICAL: Different idx for ordering
        repo_ids: vec![],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-03T00:00:00Z".to_string()),
        updated_at: Some("2024-01-03T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&child_note2).await.unwrap();

    // Export and import
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify parent_id and idx are preserved
    let imported_parent = db2.notes().get("note0001").await.unwrap();
    assert_eq!(
        imported_parent.parent_id, None,
        "Parent should have no parent_id"
    );
    assert_eq!(
        imported_parent.idx,
        Some(1),
        "Parent idx should be preserved"
    );

    let imported_child = db2.notes().get("note0002").await.unwrap();
    assert_eq!(
        imported_child.parent_id,
        Some("note0001".to_string()),
        "CRITICAL: child parent_id must be preserved during export/import!"
    );
    assert_eq!(
        imported_child.idx,
        Some(2),
        "CRITICAL: child idx must be preserved during export/import!"
    );

    let imported_child2 = db2.notes().get("note0003").await.unwrap();
    assert_eq!(
        imported_child2.parent_id,
        Some("note0001".to_string()),
        "CRITICAL: second child parent_id must be preserved!"
    );
    assert_eq!(
        imported_child2.idx,
        Some(1),
        "CRITICAL: second child idx must be preserved!"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_second_export_keeps_full_replica() {
    use context_core::{Task, TaskRepository, TaskStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let project = Project {
        id: "projfull".to_string(),
        title: "Full Replica Project".to_string(),
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
    db1.projects().create(&project).await.unwrap();

    let task_list = TaskList {
        id: "listfull".to_string(),
        title: "Full Replica List".to_string(),
        description: None,
        notes: None,
        project_id: "projfull".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db1.task_lists().create(&task_list).await.unwrap();

    let first_task = Task {
        id: "taskfull".to_string(),
        list_id: "listfull".to_string(),
        parent_id: None,
        title: "First Task".to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.tasks().create(&first_task).await.unwrap();

    db1.sync().export_all(temp_dir.path()).await.unwrap();

    // SyncManager advances the cutoff after a successful export. The next
    // export must still write the unchanged project, list, and first task.
    db1.sync()
        .set_last_synced_at(&context_core::current_timestamp())
        .await
        .unwrap();

    let second_task = Task {
        id: "taskful2".to_string(),
        list_id: "listfull".to_string(),
        parent_id: None,
        title: "Second Task".to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2024-01-02T00:00:00Z".to_string()),
        updated_at: Some("2024-01-02T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.tasks().create(&second_task).await.unwrap();

    db1.sync().export_all(temp_dir.path()).await.unwrap();

    db2.sync().import_all(temp_dir.path()).await.unwrap();

    let imported_project = db2.projects().get("projfull").await.unwrap();
    assert_eq!(imported_project.title, "Full Replica Project");

    let imported_first = db2.tasks().get("taskfull").await.unwrap();
    assert_eq!(imported_first.title, "First Task");

    let imported_second = db2.tasks().get("taskful2").await.unwrap();
    assert_eq!(imported_second.title, "Second Task");
}
