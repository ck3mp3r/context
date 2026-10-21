//! Tests for SyncRepository export of task transitions.

use crate::SqliteDatabase;
use context_core::{
    Database, HasProjects, HasSync, HasTaskLists, HasTasks, Project, ProjectRepository,
    SyncRepository, TaskList, TaskListRepository, TaskListStatus,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_includes_task_transitions() {
    // RED: Will fail because task_transition_log.jsonl not exported yet
    use context_core::{TaskRepository, TaskStatus};

    let db = setup_test_db().await;
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
    db.projects().create(&project).await.unwrap();

    // Create a task list
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
    db.task_lists().create(&task_list).await.unwrap();

    // Create task (will log initial backlog state)
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
    db.tasks().create(&task).await.unwrap();

    // Transition the task to in_progress
    let mut updated_task = task.clone();
    updated_task.status = TaskStatus::InProgress;
    db.tasks().update(&updated_task).await.unwrap();

    // Transition to done
    updated_task.status = TaskStatus::Done;
    db.tasks().update(&updated_task).await.unwrap();

    // Export
    let export_summary = db.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.tasks, 1, "Should export 1 task");

    // Verify task_transition_log.jsonl exists
    let transitions_file = temp_dir.path().join("task_transition_log.jsonl");
    assert!(
        transitions_file.exists(),
        "task_transition_log.jsonl should exist after export"
    );

    // Read and verify transitions
    use context_core::TransitionLog;
    let transitions: Vec<TransitionLog> = context_core::read_jsonl(&transitions_file).unwrap();

    assert_eq!(
        transitions.len(),
        3,
        "Should have 3 transitions: backlog -> in_progress -> done"
    );

    // Verify all transitions are for our task
    for transition in &transitions {
        assert_eq!(
            transition.task_id, "task0001",
            "All transitions should be for task0001"
        );
    }

    // Verify statuses (order not guaranteed, but presence is)
    let statuses: Vec<TaskStatus> = transitions.iter().map(|t| t.status.clone()).collect();
    assert!(
        statuses.contains(&TaskStatus::Backlog),
        "Should have backlog transition"
    );
    assert!(
        statuses.contains(&TaskStatus::InProgress),
        "Should have in_progress transition"
    );
    assert!(
        statuses.contains(&TaskStatus::Done),
        "Should have done transition"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_transitions_single_query() {
    use context_core::{TaskRepository, TaskStatus, TransitionLog};

    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

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
    db.projects().create(&project).await.unwrap();

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
    db.task_lists().create(&task_list).await.unwrap();

    // Create 3 tasks, each with 3 transitions (backlog -> in_progress -> done)
    for index in 1..=3 {
        let task = context_core::Task {
            id: format!("task000{}", index),
            list_id: "list0001".to_string(),
            parent_id: None,
            title: format!("Task {}", index),
            description: None,
            status: TaskStatus::Backlog,
            priority: None,
            tags: vec![],
            external_refs: vec![],
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-01T10:00:00Z".to_string()),
            deleted_at: None,
        };
        db.tasks().create(&task).await.unwrap();

        let mut updated = task.clone();
        updated.status = TaskStatus::InProgress;
        db.tasks().update(&updated).await.unwrap();

        updated.status = TaskStatus::Done;
        db.tasks().update(&updated).await.unwrap();
    }

    let export_summary = db.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.tasks, 3, "Should export 3 tasks");
    assert_eq!(
        export_summary.transitions, 9,
        "Should export 9 transitions (3 per task)"
    );

    let transitions_file = temp_dir.path().join("task_transition_log.jsonl");
    let transitions: Vec<TransitionLog> = context_core::read_jsonl(&transitions_file).unwrap();
    assert_eq!(
        transitions.len(),
        9,
        "JSONL should contain all 9 transitions"
    );

    for index in 1..=3 {
        let task_id = format!("task000{}", index);
        let count = transitions
            .iter()
            .filter(|transition| transition.task_id == task_id)
            .count();
        assert_eq!(count, 3, "task {} should have 3 transitions", task_id);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_excludes_transitions_of_tombstoned_tasks() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    sqlx::query(
        "INSERT INTO project (id, title, tags, created_at, updated_at)
         VALUES ('projtrns', 'Transitions', '[]', datetime('now'), datetime('now'))",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO task_list (id, project_id, title, status, tags, created_at, updated_at)
         VALUES ('listtrns', 'projtrns', 'List', 'active', '[]', datetime('now'), datetime('now'))",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO task (id, list_id, title, status, tags, external_refs, created_at, updated_at)
         VALUES ('tasktrns', 'listtrns', 'Task', 'backlog', '[]', '[]', datetime('now'), datetime('now'))",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO task_transition_log (id, task_id, status, transitioned_at)
         VALUES ('trns0001', 'tasktrns', 'backlog', datetime('now'))",
    )
    .execute(db.pool())
    .await
    .unwrap();

    // Soft-deleting the project cascades to the task list and task but
    // leaves the transition row in place.
    db.projects().delete("projtrns").await.unwrap();

    db.sync().export_all(temp_dir.path()).await.unwrap();

    let contents = std::fs::read_to_string(temp_dir.path().join("task_transition_log.jsonl"))
        .expect("task_transition_log.jsonl should exist");
    assert!(
        !contents.contains("trns0001"),
        "transitions of tombstoned tasks must not be exported, got: {}",
        contents
    );
}
