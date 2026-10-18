//! Tests for SqliteTaskListRepository restore.

use crate::SqliteDatabase;
use crate::helpers::{read_deleted_at, read_updated_at};
use context_core::{
    Database, HasProjects, HasTaskLists, HasTasks, Project, ProjectRepository, Task, TaskList,
    TaskListRepository, TaskListStatus, TaskRepository, TaskStatus,
};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

async fn create_test_project(db: &SqliteDatabase, id: &str) -> Project {
    let project = Project {
        id: id.to_string(),
        title: format!("Project {}", id),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    db.projects().create(&project).await.unwrap();
    project
}

fn make_task_list(id: &str, project_id: &str, title: &str) -> TaskList {
    TaskList {
        id: id.to_string(),
        title: title.to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project_id.to_string(),
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
async fn restore_task_list_restores_tasks() {
    let db = setup_db().await;
    create_test_project(&db, "projrst1").await;
    let task_lists = db.task_lists();

    task_lists
        .create(&make_task_list("listrst1", "projrst1", "Restore List"))
        .await
        .unwrap();
    db.tasks()
        .create(&make_task("tskrst01", "listrst1", "Restore Task"))
        .await
        .unwrap();

    task_lists.delete("listrst1").await.unwrap();
    task_lists
        .restore("listrst1")
        .await
        .expect("Restore should succeed");

    assert!(
        read_deleted_at(db.pool(), "task_list", "listrst1")
            .await
            .is_none(),
        "restore should clear the task list's deleted_at"
    );
    assert!(
        read_deleted_at(db.pool(), "task", "tskrst01")
            .await
            .is_none(),
        "restore should clear the cascade-deleted task's deleted_at"
    );
    assert_ne!(
        read_updated_at(db.pool(), "task_list", "listrst1").await,
        Some("2025-01-01 00:00:00".to_string()),
        "restore should bump the task list's updated_at"
    );
    assert_ne!(
        read_updated_at(db.pool(), "task", "tskrst01").await,
        Some("2025-01-01 00:00:00".to_string()),
        "restore should bump the task's updated_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_task_list_leaves_earlier_deleted_tasks() {
    let db = setup_db().await;
    create_test_project(&db, "projrst2").await;
    let task_lists = db.task_lists();

    task_lists
        .create(&make_task_list("listrst2", "projrst2", "Restore List"))
        .await
        .unwrap();
    db.tasks()
        .create(&make_task("tskrst02", "listrst2", "Earlier Deleted Task"))
        .await
        .unwrap();

    // Simulate an independent delete that happened before the list's delete.
    db.tasks().delete("tskrst02").await.unwrap();
    sqlx::query("UPDATE task SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("tskrst02")
        .execute(db.pool())
        .await
        .unwrap();

    task_lists.delete("listrst2").await.unwrap();
    task_lists
        .restore("listrst2")
        .await
        .expect("Restore should succeed");

    assert!(
        read_deleted_at(db.pool(), "task_list", "listrst2")
            .await
            .is_none(),
        "the task list should be restored"
    );
    assert!(
        read_deleted_at(db.pool(), "task", "tskrst02")
            .await
            .is_some(),
        "a task deleted before the list's cascade must stay deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_task_list_with_deleted_project_returns_validation() {
    let db = setup_db().await;
    create_test_project(&db, "projrst3").await;
    let task_lists = db.task_lists();

    task_lists
        .create(&make_task_list("listrst3", "projrst3", "Orphan List"))
        .await
        .unwrap();

    db.projects().delete("projrst3").await.unwrap();

    let result = task_lists.restore("listrst3").await;

    match result {
        Err(context_core::DbError::Validation { message }) => {
            assert!(
                message.contains("projrst3"),
                "validation message should name the deleted project, got: {}",
                message
            );
        }
        other => panic!(
            "restoring a task list under a deleted project should return Validation, got: {:?}",
            other
        ),
    }

    assert!(
        read_deleted_at(db.pool(), "task_list", "listrst3")
            .await
            .is_some(),
        "a refused restore must leave the task list soft-deleted"
    );
}
