use crate::SqliteDatabase;
use crate::helpers::{read_deleted_at, read_updated_at};
use context_core::{
    Database, HasTaskLists, HasTasks, Task, TaskList, TaskListRepository, TaskListStatus,
    TaskRepository, TaskStatus,
};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    // Create a test project with known ID for tests
    sqlx::query("INSERT OR IGNORE INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("test0000")
        .bind("Test Project")
        .bind("Default project for tests")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Create test project should succeed");

    db
}

fn make_task_list(id: &str, title: &str) -> TaskList {
    TaskList {
        id: id.to_string(),
        title: title.to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: "test0000".to_string(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    }
}

fn make_task(id: &str, list_id: &str, title: &str) -> Task {
    Task {
        id: id.to_string(),
        list_id: list_id.to_string(),
        parent_id: None,
        title: title.to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_task_clears_deleted_at() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listrst1", "Restore Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();
    tasks
        .create(&make_task("tskrstr1", "listrst1", "Restore Me"))
        .await
        .expect("Create task");

    tasks.delete("tskrstr1").await.expect("Delete task");
    tasks.restore("tskrstr1").await.expect("Restore task");

    assert_eq!(
        read_deleted_at(db.pool(), "task", "tskrstr1").await,
        None,
        "restore should clear the task's deleted_at"
    );
    assert_ne!(
        read_updated_at(db.pool(), "task", "tskrstr1").await,
        Some("2025-01-01 00:00:00".to_string()),
        "restore should bump the task's updated_at"
    );
    tasks
        .get("tskrstr1")
        .await
        .expect("restored task should be readable");
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_task_restores_descendants() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listrst2", "Restore Cascade Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();
    tasks
        .create(&make_task("tskpar02", "listrst2", "Parent"))
        .await
        .expect("Create parent");

    let mut child = make_task("tskchi02", "listrst2", "Child");
    child.parent_id = Some("tskpar02".to_string());
    tasks.create(&child).await.expect("Create child");

    tasks.delete("tskpar02").await.expect("Delete parent");
    tasks.restore("tskpar02").await.expect("Restore parent");

    assert_eq!(
        read_deleted_at(db.pool(), "task", "tskpar02").await,
        None,
        "restore should clear the parent's deleted_at"
    );
    assert_eq!(
        read_deleted_at(db.pool(), "task", "tskchi02").await,
        None,
        "restore should clear the cascade-deleted child's deleted_at"
    );
    assert_ne!(
        read_updated_at(db.pool(), "task", "tskchi02").await,
        Some("2025-01-01 00:00:00".to_string()),
        "restore should bump the child's updated_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_task_leaves_earlier_deleted_descendants() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listrst3", "Restore Earlier Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();
    tasks
        .create(&make_task("tskpar03", "listrst3", "Parent"))
        .await
        .expect("Create parent");

    let mut earlier = make_task("tskchi03", "listrst3", "Earlier Deleted Child");
    earlier.parent_id = Some("tskpar03".to_string());
    tasks.create(&earlier).await.expect("Create earlier child");

    let mut cascaded = make_task("tskchi04", "listrst3", "Cascade Deleted Child");
    cascaded.parent_id = Some("tskpar03".to_string());
    tasks
        .create(&cascaded)
        .await
        .expect("Create cascaded child");

    // Simulate an independent delete that happened before the parent's delete.
    tasks.delete("tskchi03").await.expect("Delete child");
    sqlx::query("UPDATE task SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("tskchi03")
        .execute(db.pool())
        .await
        .expect("Backdate child tombstone");

    tasks.delete("tskpar03").await.expect("Delete parent");
    tasks.restore("tskpar03").await.expect("Restore parent");

    assert_eq!(
        read_deleted_at(db.pool(), "task", "tskpar03").await,
        None,
        "the parent should be restored"
    );
    assert!(
        read_deleted_at(db.pool(), "task", "tskchi03")
            .await
            .is_some(),
        "a descendant deleted before the parent's cascade must stay deleted"
    );
    assert_eq!(
        read_deleted_at(db.pool(), "task", "tskchi04").await,
        None,
        "a descendant deleted by the parent's cascade must be restored"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_task_with_deleted_list_returns_validation() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listrst4", "Deleted List Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();
    tasks
        .create(&make_task("tskrstr4", "listrst4", "Orphan Task"))
        .await
        .expect("Create task");

    db.task_lists()
        .delete("listrst4")
        .await
        .expect("Delete task list");

    let result = tasks.restore("tskrstr4").await;

    match result {
        Err(context_core::DbError::Validation { message }) => {
            assert!(
                message.contains("listrst4"),
                "validation message should name the deleted task list, got: {}",
                message
            );
        }
        other => panic!(
            "restoring a task under a deleted task list should return Validation, got: {:?}",
            other
        ),
    }

    assert!(
        read_deleted_at(db.pool(), "task", "tskrstr4")
            .await
            .is_some(),
        "a refused restore must leave the task soft-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_nonexistent_task_returns_not_found() {
    let db = setup_db().await;

    let result = db.tasks().restore("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "restoring a missing task should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_live_task_is_noop() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listrst5", "Restore Noop Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();
    tasks
        .create(&make_task("tskrstr5", "listrst5", "Live Task"))
        .await
        .expect("Create task");

    let before = read_updated_at(db.pool(), "task", "tskrstr5").await;

    tasks
        .restore("tskrstr5")
        .await
        .expect("Restoring a live task should be a no-op");

    assert_eq!(
        read_deleted_at(db.pool(), "task", "tskrstr5").await,
        None,
        "a live task must stay live"
    );
    assert_eq!(
        read_updated_at(db.pool(), "task", "tskrstr5").await,
        before,
        "a no-op restore must not bump updated_at"
    );
}
