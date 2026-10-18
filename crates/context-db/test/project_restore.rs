//! Tests for SqliteProjectRepository restore.

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

fn make_project(id: &str, title: &str) -> Project {
    Project {
        id: id.to_string(),
        title: title.to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    }
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
async fn restore_project_restores_cascade_children() {
    let db = setup_db().await;
    let projects = db.projects();

    projects
        .create(&make_project("rstprj01", "Restore Project"))
        .await
        .unwrap();
    db.task_lists()
        .create(&make_task_list("rstlst01", "rstprj01", "Restore List"))
        .await
        .unwrap();
    db.tasks()
        .create(&make_task("rsttsk01", "rstlst01", "Restore Task"))
        .await
        .unwrap();

    projects.delete("rstprj01").await.unwrap();
    projects
        .restore("rstprj01")
        .await
        .expect("Restore should succeed");

    assert!(
        read_deleted_at(db.pool(), "project", "rstprj01")
            .await
            .is_none(),
        "restore should clear the project's deleted_at"
    );
    assert!(
        read_deleted_at(db.pool(), "task_list", "rstlst01")
            .await
            .is_none(),
        "restore should clear the cascade-deleted task list's deleted_at"
    );
    assert!(
        read_deleted_at(db.pool(), "task", "rsttsk01")
            .await
            .is_none(),
        "restore should clear the cascade-deleted task's deleted_at"
    );
    assert_ne!(
        read_updated_at(db.pool(), "project", "rstprj01").await,
        Some("2025-01-01 00:00:00".to_string()),
        "restore should bump the project's updated_at"
    );
    assert_ne!(
        read_updated_at(db.pool(), "task_list", "rstlst01").await,
        Some("2025-01-01 00:00:00".to_string()),
        "restore should bump the task list's updated_at"
    );
    assert_ne!(
        read_updated_at(db.pool(), "task", "rsttsk01").await,
        Some("2025-01-01 00:00:00".to_string()),
        "restore should bump the task's updated_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_project_leaves_earlier_deleted_children() {
    let db = setup_db().await;
    let projects = db.projects();

    projects
        .create(&make_project("rstprj02", "Restore Project"))
        .await
        .unwrap();
    db.task_lists()
        .create(&make_task_list("rstlst02", "rstprj02", "Restore List"))
        .await
        .unwrap();
    db.tasks()
        .create(&make_task("rsttsk02", "rstlst02", "Earlier Deleted Task"))
        .await
        .unwrap();

    // Simulate an independent delete that happened before the project's delete.
    db.tasks().delete("rsttsk02").await.unwrap();
    sqlx::query("UPDATE task SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("rsttsk02")
        .execute(db.pool())
        .await
        .unwrap();

    projects.delete("rstprj02").await.unwrap();
    projects
        .restore("rstprj02")
        .await
        .expect("Restore should succeed");

    assert!(
        read_deleted_at(db.pool(), "task_list", "rstlst02")
            .await
            .is_none(),
        "the cascade-deleted task list should be restored"
    );
    assert!(
        read_deleted_at(db.pool(), "task", "rsttsk02")
            .await
            .is_some(),
        "a task deleted before the project's cascade must stay deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_project_leaves_tasks_in_deleted_lists() {
    let db = setup_db().await;
    let projects = db.projects();

    projects
        .create(&make_project("rstprj03", "Restore Project"))
        .await
        .unwrap();
    db.task_lists()
        .create(&make_task_list(
            "rstlst03",
            "rstprj03",
            "Earlier Deleted List",
        ))
        .await
        .unwrap();

    // The list is deleted independently before the project's delete.
    db.task_lists().delete("rstlst03").await.unwrap();
    sqlx::query("UPDATE task_list SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("rstlst03")
        .execute(db.pool())
        .await
        .unwrap();

    // Task creation does not validate list liveness, so a live task can sit in
    // a deleted list. The project's delete cascade tombstones it at the
    // project's timestamp.
    db.tasks()
        .create(&make_task("rsttsk03", "rstlst03", "Task In Deleted List"))
        .await
        .unwrap();

    projects.delete("rstprj03").await.unwrap();
    projects
        .restore("rstprj03")
        .await
        .expect("Restore should succeed");

    assert!(
        read_deleted_at(db.pool(), "task_list", "rstlst03")
            .await
            .is_some(),
        "a task list deleted before the project's cascade must stay deleted"
    );
    assert!(
        read_deleted_at(db.pool(), "task", "rsttsk03")
            .await
            .is_some(),
        "a task whose list is still deleted must stay deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_nonexistent_project_returns_not_found() {
    let db = setup_db().await;

    let result = db.projects().restore("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "restoring a missing project should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_live_project_is_noop() {
    let db = setup_db().await;
    let projects = db.projects();

    projects
        .create(&make_project("rstlive1", "Live Project"))
        .await
        .unwrap();

    projects
        .restore("rstlive1")
        .await
        .expect("Restoring a live project should be a no-op");

    assert!(
        read_deleted_at(db.pool(), "project", "rstlive1")
            .await
            .is_none(),
        "deleted_at should stay NULL"
    );
    assert_eq!(
        read_updated_at(db.pool(), "project", "rstlive1").await,
        Some("2025-01-01 00:00:00".to_string()),
        "a no-op restore must not bump updated_at"
    );
}
