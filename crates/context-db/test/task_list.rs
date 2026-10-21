//! Tests for SqliteTaskListRepository FTS5 search.

use crate::SqliteDatabase;
use crate::helpers::{count_rows, read_deleted_at, read_updated_at};
use context_core::{
    Database, HasProjects, HasTaskLists, HasTasks, Project, ProjectRepository, Task, TaskList,
    TaskListQuery, TaskListRepository, TaskListStatus, TaskRepository, TaskStatus,
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

// =============================================================================
// FTS5 Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_list_by_title() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Rust Backend Development".to_string(),
        description: Some("API development".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Python Data Pipeline".to_string(),
        description: Some("ETL processing".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search by title
    let result = repo
        .search("rust", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Rust Backend Development");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_list_by_description() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Project Alpha".to_string(),
        description: Some("Machine learning research project".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Project Beta".to_string(),
        description: Some("Frontend web application".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search by description
    let result = repo
        .search("machine learning", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Project Alpha");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_list_by_notes() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Feature Implementation".to_string(),
        description: Some("Core features".to_string()),
        notes: Some("Critical for Q1 2025 release deadline".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Bug Fixes".to_string(),
        description: Some("Address technical debt".to_string()),
        notes: Some("Nice to have for Q2 2025".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search by notes
    let result = repo
        .search("critical deadline", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Feature Implementation");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_list_by_tags() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists with tags
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Frontend Sprint".to_string(),
        description: None,
        notes: None,
        tags: vec!["react".to_string(), "typescript".to_string()],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Backend Sprint".to_string(),
        description: None,
        notes: None,
        tags: vec!["rust".to_string(), "api".to_string()],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search by tag
    let result = repo
        .search("typescript", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Frontend Sprint");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_list_by_external_refs() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists with external refs
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "GitHub Integration".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_refs: vec!["owner/repo#123".to_string(), "owner/repo#456".to_string()],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Jira Integration".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_refs: vec!["PROJ-789".to_string()],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search by external ref
    let result = repo
        .search("owner/repo#123", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "GitHub Integration");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_boolean_operators() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Rust Web API".to_string(),
        description: Some("Backend service".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Rust CLI Tool".to_string(),
        description: Some("Command line interface".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0003".to_string(),
        title: "Python API".to_string(),
        description: Some("Backend service".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:02".to_string()),
        updated_at: Some("2025-01-01 00:00:02".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search with AND operator
    let result = repo
        .search("rust AND backend", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Rust Web API");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_phrase_query() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Backend API Service".to_string(),
        description: Some("RESTful API implementation".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "API Documentation".to_string(),
        description: Some("Service documentation for API".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search with exact phrase
    let result = repo
        .search("\"API implementation\"", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Backend API Service");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_special_characters() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create a task list
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: Some("Test data".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    })
    .await
    .unwrap();

    // Should sanitize special chars and return empty results
    let result = repo
        .search("test@#$%", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed with sanitization");

    // Should match "test" after sanitization
    assert_eq!(result.items.len(), 1);
}

// =============================================================================
// Soft-delete Tests
// =============================================================================

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
async fn task_list_delete_soft_deletes_and_cascades_to_tasks() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0002").await;
    let task_lists = db.task_lists();

    task_lists
        .create(&TaskList {
            id: "listdel1".to_string(),
            title: "Delete List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: project.id.clone(),
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
            archived_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    db.tasks()
        .create(&make_task("tskdel01", "listdel1", "Task To Delete"))
        .await
        .unwrap();

    task_lists.delete("listdel1").await.unwrap();

    assert!(
        read_deleted_at(db.pool(), "task_list", "listdel1")
            .await
            .is_some(),
        "task list should be soft-deleted"
    );
    assert_ne!(
        read_updated_at(db.pool(), "task_list", "listdel1").await,
        Some("2025-01-01 00:00:00".to_string()),
        "updated_at should be bumped by the soft-delete"
    );
    assert!(
        read_deleted_at(db.pool(), "task", "tskdel01")
            .await
            .is_some(),
        "task in the list should be soft-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "task_list_repo", "task_list_id = ?", "listdel1").await,
        0,
        "task_list_repo rows should be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_already_deleted_is_noop() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0003").await;
    let task_lists = db.task_lists();

    task_lists
        .create(&TaskList {
            id: "listnoop".to_string(),
            title: "Noop List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: project.id.clone(),
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
            archived_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    task_lists.delete("listnoop").await.unwrap();
    let first_deleted_at = read_deleted_at(db.pool(), "task_list", "listnoop").await;
    let first_updated_at = read_updated_at(db.pool(), "task_list", "listnoop").await;

    task_lists
        .delete("listnoop")
        .await
        .expect("Second delete should be a no-op");

    assert_eq!(
        read_deleted_at(db.pool(), "task_list", "listnoop").await,
        first_deleted_at,
        "deleted_at should not change on a repeated delete"
    );
    assert_eq!(
        read_updated_at(db.pool(), "task_list", "listnoop").await,
        first_updated_at,
        "updated_at should not be bumped on a repeated delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_nonexistent_task_list_returns_not_found() {
    let db = setup_db().await;

    let result = db.task_lists().delete("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "deleting a missing task list should return NotFound, got: {:?}",
        result
    );
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

#[tokio::test(flavor = "multi_thread")]
async fn get_soft_deleted_returns_not_found() {
    let db = setup_db().await;
    create_test_project(&db, "proj0004").await;
    let task_lists = db.task_lists();

    task_lists
        .create(&make_task_list("listdel2", "proj0004", "Deleted List"))
        .await
        .expect("Create should succeed");
    task_lists
        .delete("listdel2")
        .await
        .expect("Delete should succeed");

    let result = task_lists.get("listdel2").await;
    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "get on a soft-deleted task list should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_excludes_soft_deleted() {
    let db = setup_db().await;
    create_test_project(&db, "proj0005").await;
    let task_lists = db.task_lists();

    task_lists
        .create(&make_task_list("listlive", "proj0005", "Live List"))
        .await
        .expect("Create should succeed");
    task_lists
        .create(&make_task_list("listdead", "proj0005", "Dead List"))
        .await
        .expect("Create should succeed");
    task_lists
        .delete("listdead")
        .await
        .expect("Delete should succeed");

    let result = task_lists.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1, "list should exclude tombstones");
    assert_eq!(result.items[0].id, "listlive");
    assert_eq!(result.total, 1, "list total should exclude tombstones");
    assert_eq!(
        task_lists.count().await.expect("Count should succeed"),
        1,
        "count should exclude tombstones"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn count_deleted_counts_tombstones_only() {
    let db = setup_db().await;
    create_test_project(&db, "projcnt1").await;
    let task_lists = db.task_lists();

    task_lists
        .create(&make_task_list("cntlive1", "projcnt1", "Live List"))
        .await
        .expect("Create should succeed");
    task_lists
        .create(&make_task_list("cntdead1", "projcnt1", "Dead List"))
        .await
        .expect("Create should succeed");

    assert_eq!(
        task_lists
            .count_deleted()
            .await
            .expect("Count deleted should succeed"),
        0,
        "count_deleted should be zero without tombstones"
    );

    task_lists
        .delete("cntdead1")
        .await
        .expect("Delete should succeed");

    assert_eq!(
        task_lists
            .count_deleted()
            .await
            .expect("Count deleted should succeed"),
        1,
        "count_deleted should count the tombstone"
    );
    assert_eq!(
        task_lists.count().await.expect("Count should succeed"),
        1,
        "count should exclude the tombstone"
    );
}

// =============================================================================
// Tombstone purge tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_removes_old_tombstones() {
    let db = setup_db().await;
    create_test_project(&db, "projprg1").await;
    let task_lists = db.task_lists();

    task_lists
        .create(&make_task_list("listprg1", "projprg1", "Old Tombstone"))
        .await
        .unwrap();
    db.tasks()
        .create(&make_task("tskprg01", "listprg1", "Child Task"))
        .await
        .unwrap();

    task_lists.delete("listprg1").await.unwrap();
    sqlx::query("UPDATE task_list SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("listprg1")
        .execute(db.pool())
        .await
        .unwrap();

    task_lists
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "task_list", "id = ?", "listprg1").await,
        0,
        "an old task list tombstone must be hard-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "task", "id = ?", "tskprg01").await,
        0,
        "tasks in a purged list must be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_preserves_recent_tombstones() {
    let db = setup_db().await;
    create_test_project(&db, "projprg2").await;
    let task_lists = db.task_lists();

    task_lists
        .create(&make_task_list("listprg2", "projprg2", "Recent Tombstone"))
        .await
        .unwrap();
    task_lists.delete("listprg2").await.unwrap();

    task_lists
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "task_list", "id = ?", "listprg2").await,
        1,
        "a tombstone inside the retention period must be kept"
    );
}
