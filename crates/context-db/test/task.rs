use crate::SqliteDatabase;
use crate::helpers::count_rows;
use context_core::{
    Database, HasTaskLists, HasTasks, PageSort, SortOrder, Task, TaskList, TaskListRepository,
    TaskListStatus, TaskQuery, TaskRepository, TaskStatus,
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
        project_id: "test0000".to_string(), // Test project (created by setup_db)
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
async fn task_list_by_parent_id() {
    let db = setup_db().await;

    // Create task list
    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listpar1", "Parent Test"))
        .await
        .expect("Create should succeed");

    let tasks = db.tasks();

    // Create parent task
    tasks
        .create(&make_task("taskpar1", "listpar1", "Parent Task"))
        .await
        .unwrap();

    // Create subtasks
    let mut subtask1 = make_task("subtask1", "listpar1", "Subtask 1");
    subtask1.parent_id = Some("taskpar1".to_string());
    tasks.create(&subtask1).await.unwrap();

    let mut subtask2 = make_task("subtask2", "listpar1", "Subtask 2");
    subtask2.parent_id = Some("taskpar1".to_string());
    tasks.create(&subtask2).await.unwrap();

    // Create another root task with no subtasks
    tasks
        .create(&make_task("taskpar2", "listpar1", "Another Root"))
        .await
        .unwrap();

    // Query subtasks using parent_id filter
    let query = TaskQuery {
        list_id: Some("listpar1".to_string()),
        parent_id: Some("taskpar1".to_string()),
        ..Default::default()
    };
    let subtasks = tasks
        .list(Some(&query))
        .await
        .expect("Query should succeed");
    assert_eq!(subtasks.items.len(), 2);
    assert_eq!(subtasks.total, 2);

    // Query with different parent - should find none
    let query = TaskQuery {
        list_id: Some("listpar1".to_string()),
        parent_id: Some("taskpar2".to_string()),
        ..Default::default()
    };
    let no_subtasks = tasks
        .list(Some(&query))
        .await
        .expect("Query should succeed");
    assert!(no_subtasks.items.is_empty());
    assert_eq!(no_subtasks.total, 0);
}

// ============================================================================
// Activity-Based Sorting Tests
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn subtask_update_does_not_update_parent_timestamp() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("activity", "Activity Test"))
        .await
        .expect("Create task list should succeed");

    // Create parent task at T0
    let mut parent = make_task("", "activity", "Parent task");
    parent.status = TaskStatus::InProgress;
    parent.created_at = Some("2026-01-01 10:00:00".to_string());
    parent.updated_at = Some("2026-01-01 10:00:00".to_string());
    let created_parent = tasks
        .create(&parent)
        .await
        .expect("Create parent should succeed");

    // Capture parent's timestamp
    let parent_timestamp_before = created_parent.updated_at.clone();

    // Sleep to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Create subtask at T1
    let mut subtask = make_task("", "activity", "Subtask");
    subtask.parent_id = Some(created_parent.id.clone());
    subtask.status = TaskStatus::InProgress;
    subtask.created_at = Some("2026-01-01 11:00:00".to_string());
    subtask.updated_at = Some("2026-01-01 11:00:00".to_string());
    let mut created_subtask = tasks
        .create(&subtask)
        .await
        .expect("Create subtask should succeed");

    // Sleep again
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Update subtask at T2
    created_subtask.title = "Updated subtask".to_string();
    created_subtask.updated_at = Some("2026-01-01 12:00:00".to_string());
    tasks
        .update(&created_subtask)
        .await
        .expect("Update subtask should succeed");

    // Verify: Parent timestamp should NOT have changed
    let parent_after = tasks
        .get(&created_parent.id)
        .await
        .expect("Get parent should succeed");

    assert_eq!(
        parent_after.updated_at, parent_timestamp_before,
        "Parent's updated_at should NOT change when subtask is updated"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn parent_tasks_sorted_by_activity_include_subtask_updates() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("sort0001", "Activity Sort Test"))
        .await
        .expect("Create task list should succeed");

    // Create parent1 at T0
    let mut parent1 = make_task("", "sort0001", "Parent 1");
    parent1.created_at = Some("2026-01-01 10:00:00".to_string());
    parent1.updated_at = Some("2026-01-01 10:00:00".to_string());
    let parent1 = tasks.create(&parent1).await.expect("Create parent1");

    // Create parent2 at T1 (more recent than parent1)
    let mut parent2 = make_task("", "sort0001", "Parent 2");
    parent2.created_at = Some("2026-01-01 11:00:00".to_string());
    parent2.updated_at = Some("2026-01-01 11:00:00".to_string());
    let parent2 = tasks.create(&parent2).await.expect("Create parent2");

    // Create subtask of parent1 at T2 (most recent activity)
    let mut subtask1 = make_task("", "sort0001", "Subtask of Parent 1");
    subtask1.parent_id = Some(parent1.id.clone());
    subtask1.created_at = Some("2026-01-01 12:00:00".to_string());
    subtask1.updated_at = Some("2026-01-01 12:00:00".to_string());
    tasks.create(&subtask1).await.expect("Create subtask1");

    // Query parent tasks sorted by updated_at DESC
    // Expected order: parent1 (last_activity=12:00), parent2 (last_activity=11:00)
    let query = TaskQuery {
        page: context_core::PageSort {
            limit: Some(10),
            offset: Some(0),
            sort_by: Some("updated_at".to_string()),
            sort_order: Some(context_core::SortOrder::Desc),
        },
        list_id: Some("sort0001".to_string()),
        parent_id: None,
        status: None,
        tags: None,
        task_type: Some("task".to_string()), // Parent tasks only
    };

    let result = tasks.list(Some(&query)).await.expect("List should succeed");

    assert_eq!(result.total, 2, "Should have 2 parent tasks");
    assert_eq!(result.items.len(), 2, "Should return 2 parent tasks");

    // Assert order: parent1 should come FIRST because subtask activity is more recent
    assert_eq!(
        result.items[0].id, parent1.id,
        "Parent 1 should be first (has most recent subtask activity at 12:00)"
    );
    assert_eq!(
        result.items[1].id, parent2.id,
        "Parent 2 should be second (last activity at 11:00)"
    );
}

// =============================================================================
// FTS5 Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_by_title() {
    let db = setup_db().await;
    let repo = db.tasks();

    // Create task list
    let list = make_task_list("list0001", "Test List");
    db.task_lists().create(&list).await.unwrap();

    // Create tasks
    repo.create(&Task {
        id: "task0001".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Implement Rust Backend API".to_string(),
        description: Some("Build REST endpoints".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Task {
        id: "task0002".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Python Data Pipeline".to_string(),
        description: Some("ETL processing".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search by title
    let result = repo
        .search("rust", Some(&TaskQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Implement Rust Backend API");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_by_description() {
    let db = setup_db().await;
    let repo = db.tasks();

    let list = make_task_list("list0001", "Test List");
    db.task_lists().create(&list).await.unwrap();

    repo.create(&Task {
        id: "task0001".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Feature Alpha".to_string(),
        description: Some("Machine learning research implementation".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Task {
        id: "task0002".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Feature Beta".to_string(),
        description: Some("Frontend web components".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search by description
    let result = repo
        .search("machine learning", Some(&TaskQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Feature Alpha");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_by_tags() {
    let db = setup_db().await;
    let repo = db.tasks();

    let list = make_task_list("list0001", "Test List");
    db.task_lists().create(&list).await.unwrap();

    repo.create(&Task {
        id: "task0001".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Frontend Task".to_string(),
        description: None,
        tags: vec!["react".to_string(), "typescript".to_string()],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Task {
        id: "task0002".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Backend Task".to_string(),
        description: None,
        tags: vec!["rust".to_string(), "api".to_string()],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search by tag
    let result = repo
        .search("typescript", Some(&TaskQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Frontend Task");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_by_external_refs() {
    let db = setup_db().await;
    let repo = db.tasks();

    let list = make_task_list("list0001", "Test List");
    db.task_lists().create(&list).await.unwrap();

    repo.create(&Task {
        id: "task0001".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Fix GitHub Issue".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec!["owner/repo#123".to_string(), "owner/repo#456".to_string()],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Task {
        id: "task0002".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Resolve Jira Ticket".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec!["PROJ-789".to_string()],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search by external ref
    let result = repo
        .search("owner/repo#123", Some(&TaskQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Fix GitHub Issue");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_boolean_operators() {
    let db = setup_db().await;
    let repo = db.tasks();

    let list = make_task_list("list0001", "Test List");
    db.task_lists().create(&list).await.unwrap();

    repo.create(&Task {
        id: "task0001".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Rust Web API".to_string(),
        description: Some("Backend service implementation".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Task {
        id: "task0002".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Rust CLI Tool".to_string(),
        description: Some("Command line utility".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Task {
        id: "task0003".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Python API".to_string(),
        description: Some("Backend service implementation".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:02".to_string()),
        updated_at: Some("2025-01-01 00:00:02".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search with AND operator
    let result = repo
        .search("rust AND backend", Some(&TaskQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Rust Web API");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_phrase_query() {
    let db = setup_db().await;
    let repo = db.tasks();

    let list = make_task_list("list0001", "Test List");
    db.task_lists().create(&list).await.unwrap();

    repo.create(&Task {
        id: "task0001".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Backend Service".to_string(),
        description: Some("RESTful API implementation".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Task {
        id: "task0002".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "API Documentation".to_string(),
        description: Some("Implementation guide for API".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search with exact phrase
    let result = repo
        .search("\"API implementation\"", Some(&TaskQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Backend Service");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_combines_with_status_filter() {
    let db = setup_db().await;
    let repo = db.tasks();

    let list = make_task_list("list0001", "Test List");
    db.task_lists().create(&list).await.unwrap();

    // Create tasks with different statuses
    repo.create(&Task {
        id: "task0001".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Rust Feature".to_string(),
        description: Some("Active work".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::InProgress,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Task {
        id: "task0002".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Rust Documentation".to_string(),
        description: Some("Completed work".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Done,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search with status filter
    let query = TaskQuery {
        status: Some("in_progress".to_string()),
        ..Default::default()
    };
    let result = repo
        .search("rust", Some(&query))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Rust Feature");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_special_characters() {
    let db = setup_db().await;
    let repo = db.tasks();

    let list = make_task_list("list0001", "Test List");
    db.task_lists().create(&list).await.unwrap();

    repo.create(&Task {
        id: "task0001".to_string(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Test Task".to_string(),
        description: Some("Test data".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskStatus::Todo,
        priority: Some(1),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Should sanitize special chars and return results
    let result = repo
        .search("test@#$%", Some(&TaskQuery::default()))
        .await
        .expect("Search should succeed with sanitization");

    // Should match "test" after sanitization
    assert_eq!(result.items.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn create_task_rejects_grandparent_nesting() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("grdlist5", "Guardrail List 5"))
        .await
        .unwrap();

    // Level 0: top-level task
    let parent = make_task("grdpar04", "grdlist5", "Parent");
    db.tasks().create(&parent).await.unwrap();

    // Level 1: subtask (ok)
    let mut subtask = make_task("grdsub04", "grdlist5", "Subtask");
    subtask.parent_id = Some("grdpar04".to_string());
    db.tasks().create(&subtask).await.unwrap();

    // Level 2: sub-subtask (must be rejected)
    let mut subsubtask = make_task("grdss001", "grdlist5", "Sub-subtask");
    subsubtask.parent_id = Some("grdsub04".to_string());
    let result = db.tasks().create(&subsubtask).await;
    assert!(result.is_err(), "Sub-subtasks must be rejected");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("level") || err.contains("subtask") || err.contains("Nesting"),
        "Error should mention depth limit: {err}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn update_task_rejects_grandparent_nesting() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("grdlist6", "Guardrail List 6"))
        .await
        .unwrap();

    let parent = make_task("grdpar05", "grdlist6", "Parent");
    db.tasks().create(&parent).await.unwrap();

    let mut subtask = make_task("grdsub05", "grdlist6", "Subtask");
    subtask.parent_id = Some("grdpar05".to_string());
    db.tasks().create(&subtask).await.unwrap();

    // A separate top-level task we'll try to re-parent under the subtask
    let mut reparented = make_task("grdflat1", "grdlist6", "Flat Task");
    db.tasks().create(&reparented).await.unwrap();

    reparented.parent_id = Some("grdsub05".to_string());
    let result = db.tasks().update(&reparented).await;
    assert!(
        result.is_err(),
        "Re-parenting under a subtask must be rejected"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_soft_deleted_returns_not_found() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listdel3", "Delete List"))
        .await
        .unwrap();
    let tasks = db.tasks();

    tasks
        .create(&make_task("taskdel1", "listdel3", "Deleted Task"))
        .await
        .unwrap();
    tasks.delete("taskdel1").await.unwrap();

    let result = tasks.get("taskdel1").await;
    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "get on a soft-deleted task should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_excludes_soft_deleted() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listdel4", "Delete List"))
        .await
        .unwrap();
    let tasks = db.tasks();

    tasks
        .create(&make_task("tasklive", "listdel4", "Live Task"))
        .await
        .unwrap();
    tasks
        .create(&make_task("taskdead", "listdel4", "Dead Task"))
        .await
        .unwrap();
    tasks.delete("taskdead").await.unwrap();

    let result = tasks.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1, "list should exclude tombstones");
    assert_eq!(result.items[0].id, "tasklive");
    assert_eq!(result.total, 1, "list total should exclude tombstones");
    assert_eq!(
        tasks.count().await.expect("Count should succeed"),
        1,
        "count should exclude tombstones"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_excludes_soft_deleted() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listfts1", "FTS List"))
        .await
        .unwrap();
    let tasks = db.tasks();

    tasks
        .create(&make_task("taskfts1", "listfts1", "Zebra Unique Task"))
        .await
        .unwrap();

    let found = tasks
        .search("Zebra", None)
        .await
        .expect("Search should succeed");
    assert_eq!(found.items.len(), 1, "live task should be searchable");

    tasks.delete("taskfts1").await.unwrap();

    let found = tasks
        .search("Zebra", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        found.items.len(),
        0,
        "FTS search must exclude soft-deleted tasks"
    );

    let fts_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM task_fts WHERE task_fts MATCH 'Zebra'")
            .fetch_one(db.pool())
            .await
            .expect("FTS query should succeed");
    assert_eq!(
        fts_count, 0,
        "soft-delete must remove the task from the FTS index"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn parent_last_activity_ignores_soft_deleted_subtasks() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listact1", "Activity List"))
        .await
        .unwrap();
    let tasks = db.tasks();

    let mut parent_a = make_task("paract01", "listact1", "Parent A");
    parent_a.updated_at = Some("2025-01-01 00:00:00".to_string());
    tasks.create(&parent_a).await.unwrap();

    let mut parent_b = make_task("paract02", "listact1", "Parent B");
    parent_b.updated_at = Some("2025-01-02 00:00:00".to_string());
    tasks.create(&parent_b).await.unwrap();

    // Live subtask of A, updated later than both parents.
    let mut child_a = make_task("chiact01", "listact1", "Child A");
    child_a.parent_id = Some("paract01".to_string());
    child_a.updated_at = Some("2025-06-01 00:00:00".to_string());
    tasks.create(&child_a).await.unwrap();

    // Subtask of B, updated even later, then soft-deleted.
    let mut child_b = make_task("chibct01", "listact1", "Child B");
    child_b.parent_id = Some("paract02".to_string());
    child_b.updated_at = Some("2025-07-01 00:00:00".to_string());
    tasks.create(&child_b).await.unwrap();
    tasks.delete("chibct01").await.unwrap();

    let query = TaskQuery {
        task_type: Some("task".to_string()),
        page: PageSort {
            sort_by: Some("updated_at".to_string()),
            sort_order: Some(SortOrder::Desc),
            ..Default::default()
        },
        ..Default::default()
    };
    let result = tasks.list(Some(&query)).await.expect("List should succeed");
    let ids: Vec<&str> = result.items.iter().map(|task| task.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["paract01", "paract02"],
        "soft-deleted subtask activity must not affect parent ordering"
    );
}

// =============================================================================
// Tombstone purge tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_removes_old_tombstones() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listprg1", "Purge List"))
        .await
        .unwrap();

    let tasks = db.tasks();
    tasks
        .create(&make_task("tskprg01", "listprg1", "Old Tombstone"))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO task_transition_log (id, task_id, status, transitioned_at) VALUES (?, ?, ?, ?)",
    )
    .bind("prglg001")
    .bind("tskprg01")
    .bind("in_progress")
    .bind("2025-01-01 00:00:00")
    .execute(db.pool())
    .await
    .unwrap();

    tasks.delete("tskprg01").await.unwrap();
    sqlx::query("UPDATE task SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("tskprg01")
        .execute(db.pool())
        .await
        .unwrap();

    tasks
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "task", "id = ?", "tskprg01").await,
        0,
        "an old task tombstone must be hard-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "task_transition_log", "task_id = ?", "tskprg01").await,
        0,
        "transition logs of purged tasks must be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_preserves_recent_tombstones() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listprg2", "Purge List"))
        .await
        .unwrap();

    let tasks = db.tasks();
    tasks
        .create(&make_task("tskprg02", "listprg2", "Recent Tombstone"))
        .await
        .unwrap();
    tasks.delete("tskprg02").await.unwrap();

    tasks
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "task", "id = ?", "tskprg02").await,
        1,
        "a tombstone inside the retention period must be kept"
    );
}
