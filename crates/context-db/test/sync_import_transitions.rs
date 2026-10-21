//! Tests for SyncRepository import of task transitions.

use crate::SqliteDatabase;
use context_core::{
    Database, HasProjects, HasSync, HasTaskLists, HasTasks, Project, ProjectRepository, Repo,
    SyncRepository, TaskList, TaskListRepository, TaskListStatus, write_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_restores_task_transitions() {
    // RED: Will fail because import doesn't restore transitions yet
    use context_core::{TaskRepository, TaskStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create project and task list in db1
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

    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        project_id: "projtest".to_string(),
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

    // Create task with transitions
    let task = context_core::Task {
        id: "task0001".to_string(),
        list_id: "list0001".to_string(),
        parent_id: None,
        title: "Test Task".to_string(),
        description: Some("Description".to_string()),
        status: TaskStatus::Backlog,
        priority: Some(2),
        tags: vec!["test".to_string()],
        external_refs: vec![],
        created_at: Some("2024-01-01T10:00:00Z".to_string()),
        updated_at: Some("2024-01-01T10:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.tasks().create(&task).await.unwrap();

    // Transition to in_progress
    let mut updated_task = task.clone();
    updated_task.status = TaskStatus::InProgress;
    db1.tasks().update(&updated_task).await.unwrap();

    // Export from db1
    db1.sync().export_all(temp_dir.path()).await.unwrap();

    // Import to db2
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify task was imported
    let imported_task = db2.tasks().get("task0001").await.unwrap();
    assert_eq!(imported_task.status, TaskStatus::InProgress);

    // Verify transitions were imported
    let transitions = db2
        .tasks()
        .get_transitions("task0001", None, None)
        .await
        .unwrap();
    assert_eq!(
        transitions.items.len(),
        2,
        "Should have 2 transitions: backlog -> in_progress"
    );

    // Verify statuses
    let statuses: Vec<TaskStatus> = transitions.items.iter().map(|t| t.status.clone()).collect();
    assert!(
        statuses.contains(&TaskStatus::Backlog),
        "Should have backlog transition"
    );
    assert!(
        statuses.contains(&TaskStatus::InProgress),
        "Should have in_progress transition"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_backwards_compatible_without_transitions() {
    // RED: Will fail if import requires task_transition_log.jsonl
    use context_core::{TaskRepository, TaskStatus};

    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create project and task list
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
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();

    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        project_id: "projtest".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    write_jsonl(&temp_dir.path().join("lists.jsonl"), &[task_list]).unwrap();

    // Create old-style task JSONL (without transitions file)
    let task = context_core::Task {
        id: "task0001".to_string(),
        list_id: "list0001".to_string(),
        parent_id: None,
        title: "Old Task".to_string(),
        description: Some("From old export".to_string()),
        status: TaskStatus::Done,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2024-01-01T10:00:00Z".to_string()),
        updated_at: Some("2024-01-01T12:00:00Z".to_string()),
        deleted_at: None,
    };
    write_jsonl(&temp_dir.path().join("tasks.jsonl"), &[task]).unwrap();

    // Write empty files for other entities
    write_jsonl::<Repo>(&temp_dir.path().join("repos.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();

    // NOTE: No task_transition_log.jsonl file - simulating old export

    // Import should succeed (backwards compatible)
    let result = db.sync().import_all(temp_dir.path()).await;
    assert!(
        result.is_ok(),
        "Import should succeed without transitions file (backward compatibility)"
    );

    // Verify task was imported
    let imported_task = db.tasks().get("task0001").await.unwrap();
    assert_eq!(imported_task.title, "Old Task");
    assert_eq!(imported_task.status, TaskStatus::Done);

    // Transitions should be empty (no transitions file existed)
    let transitions = db
        .tasks()
        .get_transitions("task0001", None, None)
        .await
        .unwrap();
    assert_eq!(
        transitions.items.len(),
        0,
        "Should have no transitions when importing old export"
    );
}
