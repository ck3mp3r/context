//! Tests for SyncRepository import of task transitions.

use crate::SqliteDatabase;
use context_core::{
    Database, HasProjects, HasSync, HasTaskLists, HasTasks, Project, ProjectRepository, Repo,
    SyncRepository, Task, TaskList, TaskListRepository, TaskListStatus, TaskStatus, TransitionLog,
    write_jsonl,
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

// ========== Compare-on-conflict guard tests ==========

fn transition(id: &str, task_id: &str, status: TaskStatus, at: &str) -> TransitionLog {
    TransitionLog {
        id: id.to_string(),
        task_id: task_id.to_string(),
        status,
        transitioned_at: at.to_string(),
    }
}

/// Write a minimal replica whose only variable part is the transition file.
fn write_transition_replica(temp_dir: &TempDir, transitions: &[TransitionLog]) {
    let project = Project {
        id: "projtrns".to_string(),
        title: "Transition Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01 00:00:00".to_string()),
        updated_at: Some("2024-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();

    let task_list = TaskList {
        id: "listtrns".to_string(),
        title: "Transition List".to_string(),
        description: None,
        notes: None,
        project_id: "projtrns".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec![],
        created_at: Some("2024-01-01 00:00:00".to_string()),
        updated_at: Some("2024-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    write_jsonl(&temp_dir.path().join("lists.jsonl"), &[task_list]).unwrap();

    let task = Task {
        id: "tasktrns".to_string(),
        list_id: "listtrns".to_string(),
        parent_id: None,
        title: "Transition Task".to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2024-01-01 00:00:00".to_string()),
        updated_at: Some("2024-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    write_jsonl(&temp_dir.path().join("tasks.jsonl"), &[task]).unwrap();
    write_jsonl(
        &temp_dir.path().join("task_transition_log.jsonl"),
        transitions,
    )
    .unwrap();
}

/// Read every transition row, ordered by id.
async fn transition_rows(db: &SqliteDatabase) -> Vec<(String, String, String, String)> {
    sqlx::query_as(
        "SELECT id, task_id, status, transitioned_at FROM task_transition_log ORDER BY id",
    )
    .fetch_all(db.pool())
    .await
    .expect("transition rows should be readable")
}

/// Count every task_transition_log row the import updates.
///
/// A trigger records the updates, so a test can tell a skipped rewrite (no
/// updates) from an executed one.
async fn install_transition_update_counter(db: &SqliteDatabase) {
    sqlx::query(
        "CREATE TABLE transition_updates (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            updates INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(db.pool())
    .await
    .expect("counter table should be created");

    sqlx::query(
        "CREATE TRIGGER task_transition_log_updates AFTER UPDATE ON task_transition_log BEGIN
            INSERT INTO transition_updates (id, updates) VALUES (1, 1)
            ON CONFLICT(id) DO UPDATE SET updates = updates + 1;
         END",
    )
    .execute(db.pool())
    .await
    .expect("counter trigger should be created");
}

async fn reset_transition_updates(db: &SqliteDatabase) {
    sqlx::query("DELETE FROM transition_updates")
        .execute(db.pool())
        .await
        .expect("counter reset should succeed");
}

async fn transition_updates(db: &SqliteDatabase) -> i64 {
    sqlx::query_scalar("SELECT updates FROM transition_updates WHERE id = 1")
        .fetch_optional(db.pool())
        .await
        .expect("counter read should succeed")
        .unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread")]
async fn import_into_empty_db_writes_every_transition_row() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let transitions = vec![
        transition(
            "trns0001",
            "tasktrns",
            TaskStatus::Backlog,
            "2024-01-01 10:00:00",
        ),
        transition(
            "trns0002",
            "tasktrns",
            TaskStatus::Todo,
            "2024-01-01 11:00:00",
        ),
        transition(
            "trns0003",
            "tasktrns",
            TaskStatus::InProgress,
            "2024-01-01 12:00:00",
        ),
    ];
    write_transition_replica(&temp_dir, &transitions);

    let summary = db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(summary.transitions, 3, "the summary counts file lines");
    assert_eq!(
        transition_rows(&db).await.len(),
        3,
        "an empty database must receive every file transition"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn import_overwrites_changed_transition_rows() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    write_transition_replica(
        &temp_dir,
        &[transition(
            "trns0001",
            "tasktrns",
            TaskStatus::Backlog,
            "2024-01-01 10:00:00",
        )],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Same id, different status and timestamp.
    write_transition_replica(
        &temp_dir,
        &[transition(
            "trns0001",
            "tasktrns",
            TaskStatus::Done,
            "2024-01-02 10:00:00",
        )],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(
        transition_rows(&db).await,
        vec![(
            "trns0001".to_string(),
            "tasktrns".to_string(),
            "done".to_string(),
            "2024-01-02 10:00:00".to_string(),
        )],
        "a changed transition must be overwritten with the file's values"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn import_skips_identical_transition_rewrites() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    install_transition_update_counter(&db).await;

    let transitions = vec![
        transition(
            "trns0001",
            "tasktrns",
            TaskStatus::Backlog,
            "2024-01-01 10:00:00",
        ),
        transition(
            "trns0002",
            "tasktrns",
            TaskStatus::Todo,
            "2024-01-01 11:00:00",
        ),
    ];
    write_transition_replica(&temp_dir, &transitions);
    db.sync().import_all(temp_dir.path()).await.unwrap();
    let before = transition_rows(&db).await;
    reset_transition_updates(&db).await;

    // The file is unchanged, so every conflicting row already holds the file's
    // values and the guard must suppress the update.
    db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(
        transition_updates(&db).await,
        0,
        "an identical re-import must not rewrite transition rows"
    );
    assert_eq!(
        transition_rows(&db).await,
        before,
        "both imports must leave identical task_transition_log contents"
    );
}
