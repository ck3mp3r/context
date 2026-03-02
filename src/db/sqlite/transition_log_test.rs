//! Tests for TransitionLog repository.

use crate::db::{Database, SqliteDatabase, TaskStatus, TransitionLog};

/// Helper to create test fixtures (project, task_list, task).
async fn setup_test_task(db: &SqliteDatabase, task_id: &str, status: &str) {
    // Create project
    sqlx::query(
        "INSERT INTO project (id, title, tags, created_at, updated_at) 
         VALUES ('proj1234', 'Test Project', '[]', datetime('now'), datetime('now'))",
    )
    .execute(db.pool())
    .await
    .expect("Failed to insert test project");

    // Create task list
    sqlx::query(
        "INSERT INTO task_list (id, project_id, title, status, tags, created_at, updated_at) 
         VALUES ('list5678', 'proj1234', 'Test List', 'active', '[]', datetime('now'), datetime('now'))"
    )
    .execute(db.pool())
    .await
    .expect("Failed to insert test task list");

    // Create task
    sqlx::query(
        "INSERT INTO task (id, list_id, title, status, tags, external_refs, created_at, updated_at) 
         VALUES (?, 'list5678', 'Test Task', ?, '[]', '[]', datetime('now'), datetime('now'))"
    )
    .bind(task_id)
    .bind(status)
    .execute(db.pool())
    .await
    .expect("Failed to insert test task");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_insert_transition_log() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().expect("Failed to run migrations");

    let task_id = "test1234";
    setup_test_task(&db, task_id, "backlog").await;

    // Insert transition log
    let transition = TransitionLog {
        id: "trans001".to_string(),
        task_id: task_id.to_string(),
        from_status: None,
        to_status: TaskStatus::Backlog,
        transitioned_at: "2026-03-02 20:00:00".to_string(),
    };

    let result = db.transition_logs().insert(&transition).await;
    assert!(result.is_ok(), "Insert should succeed");

    // Verify it was inserted
    let transitions = db.transition_logs().list_by_task_id(task_id).await.unwrap();
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].id, "trans001");
    assert_eq!(transitions[0].task_id, task_id);
    assert_eq!(transitions[0].from_status, None);
    assert_eq!(transitions[0].to_status, TaskStatus::Backlog);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_by_task_id_ordered() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().expect("Failed to run migrations");

    let task_id = "test1234";
    setup_test_task(&db, task_id, "in_progress").await;

    // Insert multiple transitions
    let transitions = vec![
        TransitionLog {
            id: "trans001".to_string(),
            task_id: task_id.to_string(),
            from_status: None,
            to_status: TaskStatus::Backlog,
            transitioned_at: "2026-03-02 20:00:00".to_string(),
        },
        TransitionLog {
            id: "trans002".to_string(),
            task_id: task_id.to_string(),
            from_status: Some(TaskStatus::Backlog),
            to_status: TaskStatus::Todo,
            transitioned_at: "2026-03-02 20:01:00".to_string(),
        },
        TransitionLog {
            id: "trans003".to_string(),
            task_id: task_id.to_string(),
            from_status: Some(TaskStatus::Todo),
            to_status: TaskStatus::InProgress,
            transitioned_at: "2026-03-02 20:02:00".to_string(),
        },
    ];

    for transition in &transitions {
        db.transition_logs().insert(transition).await.unwrap();
    }

    // List transitions - should be ordered by transitioned_at DESC (newest first)
    let result = db.transition_logs().list_by_task_id(task_id).await.unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, "trans003"); // Newest first
    assert_eq!(result[1].id, "trans002");
    assert_eq!(result[2].id, "trans001"); // Oldest last
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_by_task_id() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().expect("Failed to run migrations");

    let task_id = "test1234";
    setup_test_task(&db, task_id, "backlog").await;

    // Insert transitions
    let transition = TransitionLog {
        id: "trans001".to_string(),
        task_id: task_id.to_string(),
        from_status: None,
        to_status: TaskStatus::Backlog,
        transitioned_at: "2026-03-02 20:00:00".to_string(),
    };
    db.transition_logs().insert(&transition).await.unwrap();

    // Delete transitions
    let result = db.transition_logs().delete_by_task_id(task_id).await;
    assert!(result.is_ok());

    // Verify deleted
    let transitions = db.transition_logs().list_by_task_id(task_id).await.unwrap();
    assert_eq!(transitions.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cascade_delete_on_task_delete() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().expect("Failed to run migrations");

    let task_id = "test1234";
    setup_test_task(&db, task_id, "backlog").await;

    // Insert transition
    let transition = TransitionLog {
        id: "trans001".to_string(),
        task_id: task_id.to_string(),
        from_status: None,
        to_status: TaskStatus::Backlog,
        transitioned_at: "2026-03-02 20:00:00".to_string(),
    };
    db.transition_logs().insert(&transition).await.unwrap();

    // Delete task (should cascade to transitions)
    sqlx::query("DELETE FROM task WHERE id = ?")
        .bind(task_id)
        .execute(db.pool())
        .await
        .expect("Failed to delete task");

    // Verify transitions were cascade deleted
    let transitions = db.transition_logs().list_by_task_id(task_id).await.unwrap();
    assert_eq!(transitions.len(), 0);
}
