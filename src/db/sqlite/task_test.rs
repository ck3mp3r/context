//! Tests for SqliteTaskRepository.

use crate::db::{
    Database, SqliteDatabase, Task, TaskList, TaskListRepository, TaskListStatus, TaskQuery,
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
        external_ref: None,
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: "test0000".to_string(), // Test project (created by setup_db)
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
        archived_at: None,
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
        created_at: Some("2025-01-01 00:00:00".to_string()),
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn task_timestamps_are_optional() {
    let db = setup_db().await;
    let list = db
        .task_lists()
        .create(&make_task_list("list0001", "Test List"))
        .await
        .expect("Create list");

    // Test 1: Provided timestamps are respected
    let task_with_timestamps = Task {
        id: String::new(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Task with timestamps".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: None,
        tags: vec![],
        created_at: Some("2025-01-15 10:00:00".to_string()),
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-15 11:00:00".to_string()),
    };

    let created_with_ts = db
        .tasks()
        .create(&task_with_timestamps)
        .await
        .expect("Create task");
    assert_eq!(
        created_with_ts.created_at,
        Some("2025-01-15 10:00:00".to_string())
    );
    assert_eq!(
        created_with_ts.updated_at,
        Some("2025-01-15 11:00:00".to_string())
    );

    // Test 2: None timestamps are auto-generated
    let task_without_timestamps = Task {
        id: String::new(),
        list_id: list.id.clone(),
        parent_id: None,
        title: "Task without timestamps".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: None,
        tags: vec![],
        created_at: None,
        started_at: None,
        completed_at: None,
        updated_at: None,
    };

    let created_without_ts = db
        .tasks()
        .create(&task_without_timestamps)
        .await
        .expect("Create task");
    assert!(created_without_ts.created_at.is_some());
    assert!(created_without_ts.updated_at.is_some());
    assert!(!created_without_ts.created_at.as_ref().unwrap().is_empty());
    assert!(!created_without_ts.updated_at.as_ref().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn task_create_and_get() {
    let db = setup_db().await;

    // Create a task list first (required FK)
    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("tasklst1", "Tasks For Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();

    let task = Task {
        id: "task0001".to_string(),
        list_id: "tasklst1".to_string(),
        parent_id: None,
        title: "Complete the implementation".to_string(),
        description: None,
        status: TaskStatus::InProgress,
        priority: Some(2),
        tags: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        started_at: Some("2025-01-02 09:00:00".to_string()),
        completed_at: None,
        updated_at: Some("2025-01-02 09:00:00".to_string()),
    };

    tasks.create(&task).await.expect("Create should succeed");

    let retrieved = tasks.get("task0001").await.expect("Get should succeed");
    assert_eq!(retrieved.id, task.id);
    assert_eq!(retrieved.list_id, task.list_id);
    assert_eq!(retrieved.title, task.title);
    assert_eq!(retrieved.status, TaskStatus::InProgress);
    assert_eq!(retrieved.priority, Some(2));
    assert_eq!(
        retrieved.started_at,
        Some("2025-01-02 09:00:00".to_string())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_get_nonexistent_returns_not_found() {
    let db = setup_db().await;
    let tasks = db.tasks();

    let result = tasks.get("nonexist").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_by_list_id() {
    let db = setup_db().await;

    // Create task lists
    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listbyl1", "List One"))
        .await
        .expect("Create should succeed");
    task_lists
        .create(&make_task_list("listbyl2", "List Two"))
        .await
        .expect("Create should succeed");

    let tasks = db.tasks();

    // Add tasks to both lists
    tasks
        .create(&make_task("taskby01", "listbyl1", "Task in list one"))
        .await
        .unwrap();
    tasks
        .create(&make_task("taskby02", "listbyl1", "Another in list one"))
        .await
        .unwrap();
    tasks
        .create(&make_task("taskby03", "listbyl2", "Task in list two"))
        .await
        .unwrap();

    // Query by list using list_id filter
    let query = TaskQuery {
        list_id: Some("listbyl1".to_string()),
        ..Default::default()
    };
    let result = tasks
        .list(Some(&query))
        .await
        .expect("Query should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    let query = TaskQuery {
        list_id: Some("listbyl2".to_string()),
        ..Default::default()
    };
    let result = tasks
        .list(Some(&query))
        .await
        .expect("Query should succeed");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.total, 1);
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

#[tokio::test(flavor = "multi_thread")]
async fn task_update() {
    let db = setup_db().await;

    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listupd2", "Update Test"))
        .await
        .expect("Create should succeed");

    let tasks = db.tasks();

    let mut task = make_task("taskupd1", "listupd2", "Original");
    tasks.create(&task).await.expect("Create should succeed");

    task.title = "Updated content".to_string();
    task.status = TaskStatus::Done;
    task.completed_at = Some("2025-01-15 17:00:00".to_string());
    task.priority = Some(1);
    tasks.update(&task).await.expect("Update should succeed");

    let retrieved = tasks.get("taskupd1").await.expect("Get should succeed");
    assert_eq!(retrieved.title, "Updated content");
    assert_eq!(retrieved.status, TaskStatus::Done);
    assert_eq!(
        retrieved.completed_at,
        Some("2025-01-15 17:00:00".to_string())
    );
    assert_eq!(retrieved.priority, Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn task_delete() {
    let db = setup_db().await;

    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listdel2", "Delete Test"))
        .await
        .expect("Create should succeed");

    let tasks = db.tasks();

    let task = make_task("taskdel1", "listdel2", "To Delete");
    tasks.create(&task).await.expect("Create should succeed");

    tasks
        .delete("taskdel1")
        .await
        .expect("Delete should succeed");

    let result = tasks.get("taskdel1").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn task_create_with_tags() {
    let db = setup_db().await;

    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listwtag", "Tags Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();

    let task = Task {
        id: "taskwtag".to_string(),
        list_id: "listwtag".to_string(),
        parent_id: None,
        title: "Task with tags".to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec!["rust".to_string(), "backend".to_string()],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    tasks.create(&task).await.expect("Create should succeed");

    let retrieved = tasks.get("taskwtag").await.expect("Get should succeed");
    assert_eq!(
        retrieved.tags,
        vec!["rust".to_string(), "backend".to_string()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_with_tag_filter() {
    let db = setup_db().await;

    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listfilt", "Filter Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();

    // Create tasks with different tags
    let mut task1 = make_task("taskfil1", "listfilt", "Rust task");
    task1.tags = vec!["rust".to_string(), "backend".to_string()];
    tasks.create(&task1).await.unwrap();

    let mut task2 = make_task("taskfil2", "listfilt", "Python task");
    task2.tags = vec!["python".to_string(), "backend".to_string()];
    tasks.create(&task2).await.unwrap();

    let mut task3 = make_task("taskfil3", "listfilt", "Frontend task");
    task3.tags = vec!["typescript".to_string(), "frontend".to_string()];
    tasks.create(&task3).await.unwrap();

    // Filter by "rust" tag - should find 1
    let query = TaskQuery {
        list_id: Some("listfilt".to_string()),
        tags: Some(vec!["rust".to_string()]),
        ..Default::default()
    };
    let results = tasks.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.total, 1); // DB-level filtering verified by total
    assert_eq!(results.items[0].title, "Rust task");

    // Filter by "backend" tag - should find 2
    let query = TaskQuery {
        list_id: Some("listfilt".to_string()),
        tags: Some(vec!["backend".to_string()]),
        ..Default::default()
    };
    let results = tasks.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(results.items.len(), 2);
    assert_eq!(results.total, 2);

    // Filter by nonexistent tag
    let query = TaskQuery {
        list_id: Some("listfilt".to_string()),
        tags: Some(vec!["nonexistent".to_string()]),
        ..Default::default()
    };
    let results = tasks.list(Some(&query)).await.expect("List should succeed");
    assert!(results.items.is_empty());
    assert_eq!(results.total, 0);
}

// =============================================================================
// Auto-timestamp tests for status transitions
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn task_update_status_to_done_sets_completed_at() {
    let db = setup_db().await;

    let task_lists = db.task_lists();
    let list = task_lists
        .create(&TaskList {
            id: String::new(), // Auto-generate
            title: "Timestamp Test".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    let tasks = db.tasks();

    // Create task in todo status
    let created = tasks
        .create(&Task {
            id: String::new(), // Auto-generate
            list_id: list.id.clone(),
            parent_id: None,
            title: "Task to complete".to_string(),
            description: None,
            status: TaskStatus::Todo,
            priority: None,
            tags: vec![],
            created_at: None,
            started_at: None,
            completed_at: None,
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        })
        .await
        .expect("Create should succeed");

    assert_eq!(created.status, TaskStatus::Todo);
    assert!(created.completed_at.is_none());

    // Update status to done
    let mut updated = created.clone();
    updated.status = TaskStatus::Done;
    tasks.update(&updated).await.expect("Update should succeed");

    // completed_at should be auto-set
    let after = tasks.get(&created.id).await.expect("Get should succeed");
    assert_eq!(after.status, TaskStatus::Done);
    assert!(
        after.completed_at.is_some(),
        "completed_at should be auto-set when status changes to done"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_update_status_to_done_twice_is_idempotent() {
    let db = setup_db().await;

    let task_lists = db.task_lists();
    let list = task_lists
        .create(&TaskList {
            id: String::new(),
            title: "Idempotent Test".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    let tasks = db.tasks();

    let created = tasks
        .create(&Task {
            id: String::new(),
            list_id: list.id.clone(),
            parent_id: None,
            title: "Task".to_string(),
            description: None,
            status: TaskStatus::Todo,
            priority: None,
            tags: vec![],
            created_at: None,
            started_at: None,
            completed_at: None,
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        })
        .await
        .expect("Create should succeed");

    // First: mark as done
    let mut first = created.clone();
    first.status = TaskStatus::Done;
    tasks.update(&first).await.expect("Update should succeed");

    let after_first = tasks.get(&created.id).await.expect("Get should succeed");
    let first_completed_at = after_first.completed_at.clone();
    assert!(first_completed_at.is_some());

    // Second: mark as done again
    let mut second = after_first.clone();
    second.status = TaskStatus::Done;
    tasks.update(&second).await.expect("Update should succeed");

    let after_second = tasks.get(&created.id).await.expect("Get should succeed");

    // completed_at should be unchanged (idempotent)
    assert_eq!(
        after_second.completed_at, first_completed_at,
        "completed_at should not change when status is already done"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_update_status_to_in_progress_sets_started_at() {
    let db = setup_db().await;

    let task_lists = db.task_lists();
    let list = task_lists
        .create(&TaskList {
            id: String::new(),
            title: "Start Test".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    let tasks = db.tasks();

    // Create task in backlog
    let created = tasks
        .create(&Task {
            id: String::new(),
            list_id: list.id.clone(),
            parent_id: None,
            title: "Task to start".to_string(),
            description: None,
            status: TaskStatus::Backlog,
            priority: None,
            tags: vec![],
            created_at: None,
            started_at: None,
            completed_at: None,
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        })
        .await
        .expect("Create should succeed");

    assert_eq!(created.status, TaskStatus::Backlog);
    assert!(created.started_at.is_none());

    // Update status to in_progress
    let mut updated = created.clone();
    updated.status = TaskStatus::InProgress;
    tasks.update(&updated).await.expect("Update should succeed");

    // started_at should be auto-set
    let after = tasks.get(&created.id).await.expect("Get should succeed");
    assert_eq!(after.status, TaskStatus::InProgress);
    assert!(
        after.started_at.is_some(),
        "started_at should be auto-set when status changes to in_progress"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_update_status_from_done_to_in_progress_clears_completed_at() {
    let db = setup_db().await;

    let task_lists = db.task_lists();
    let list = task_lists
        .create(&TaskList {
            id: String::new(),
            title: "Revert Test".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    let tasks = db.tasks();

    // Create task that's already done
    let created = tasks
        .create(&Task {
            id: String::new(),
            list_id: list.id.clone(),
            parent_id: None,
            title: "Done task".to_string(),
            description: None,
            status: TaskStatus::Done,
            priority: None,
            tags: vec![],
            created_at: None,
            started_at: None,
            completed_at: Some("2025-01-01 12:00:00".to_string()),
            updated_at: Some("2025-01-01 12:00:00".to_string()),
        })
        .await
        .expect("Create should succeed");

    assert_eq!(created.status, TaskStatus::Done);
    assert!(created.completed_at.is_some());

    // Revert to in_progress
    let mut updated = created.clone();
    updated.status = TaskStatus::InProgress;
    tasks.update(&updated).await.expect("Update should succeed");

    // completed_at should be cleared
    let after = tasks.get(&created.id).await.expect("Get should succeed");
    assert_eq!(after.status, TaskStatus::InProgress);
    assert!(
        after.completed_at.is_none(),
        "completed_at should be cleared when reverting from done"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_update_other_fields_preserves_timestamps() {
    let db = setup_db().await;

    let task_lists = db.task_lists();
    let list = task_lists
        .create(&TaskList {
            id: String::new(),
            title: "Preserve Test".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    let tasks = db.tasks();

    // Create task that's in progress
    let created = tasks
        .create(&Task {
            id: String::new(),
            list_id: list.id.clone(),
            parent_id: None,
            title: "Active task".to_string(),
            description: None,
            status: TaskStatus::InProgress,
            priority: None,
            tags: vec![],
            created_at: None,
            started_at: Some("2025-01-01 10:00:00".to_string()),
            completed_at: None,
            updated_at: Some("2025-01-01 10:00:00".to_string()),
        })
        .await
        .expect("Create should succeed");

    let original_started_at = created.started_at.clone();

    // Update other fields (title, priority) without changing status
    let mut updated = created.clone();
    updated.title = "Updated content".to_string();
    updated.priority = Some(5);
    tasks.update(&updated).await.expect("Update should succeed");

    // started_at should be preserved
    let after = tasks.get(&created.id).await.expect("Get should succeed");
    assert_eq!(after.title, "Updated content");
    assert_eq!(after.priority, Some(5));
    assert_eq!(
        after.started_at, original_started_at,
        "started_at should be preserved when status doesn't change"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_stats_for_list_returns_counts_by_status() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Create task list
    task_lists
        .create(&make_task_list("statlist", "Stats Test"))
        .await
        .expect("Create task list should succeed");

    // Create tasks with different statuses
    let mut task1 = make_task("stat0001", "statlist", "Backlog task");
    task1.status = TaskStatus::Backlog;
    tasks.create(&task1).await.unwrap();

    let mut task2 = make_task("stat0002", "statlist", "Todo task");
    task2.status = TaskStatus::Todo;
    tasks.create(&task2).await.unwrap();

    let mut task3 = make_task("stat0003", "statlist", "Another todo");
    task3.status = TaskStatus::Todo;
    tasks.create(&task3).await.unwrap();

    let mut task4 = make_task("stat0004", "statlist", "In progress");
    task4.status = TaskStatus::InProgress;
    tasks.create(&task4).await.unwrap();

    let mut task5 = make_task("stat0005", "statlist", "Done task");
    task5.status = TaskStatus::Done;
    tasks.create(&task5).await.unwrap();

    let mut task6 = make_task("stat0006", "statlist", "Another done");
    task6.status = TaskStatus::Done;
    tasks.create(&task6).await.unwrap();

    let mut task7 = make_task("stat0007", "statlist", "Another done 2");
    task7.status = TaskStatus::Done;
    tasks.create(&task7).await.unwrap();

    // Get stats
    let stats = tasks
        .get_stats_for_list("statlist")
        .await
        .expect("Get stats should succeed");

    assert_eq!(stats.list_id, "statlist");
    assert_eq!(stats.total, 7);
    assert_eq!(stats.backlog, 1);
    assert_eq!(stats.todo, 2);
    assert_eq!(stats.in_progress, 1);
    assert_eq!(stats.review, 0);
    assert_eq!(stats.done, 3);
    assert_eq!(stats.cancelled, 0);
}

// =============================================================================
// Cascade status update tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn cascade_status_to_all_matching_subtasks() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup: Create task list
    task_lists
        .create(&make_task_list("casclist", "Cascade Test"))
        .await
        .expect("Create task list should succeed");

    // Create parent task (status: backlog)
    let mut parent = make_task("parent01", "casclist", "Parent task");
    parent.status = TaskStatus::Backlog;
    tasks
        .create(&parent)
        .await
        .expect("Create parent should succeed");

    // Create 3 subtasks (all status: backlog, matching parent)
    let mut subtask1 = make_task("subtsk01", "casclist", "Subtask 1");
    subtask1.parent_id = Some("parent01".to_string());
    subtask1.status = TaskStatus::Backlog;
    tasks
        .create(&subtask1)
        .await
        .expect("Create subtask1 should succeed");

    let mut subtask2 = make_task("subtsk02", "casclist", "Subtask 2");
    subtask2.parent_id = Some("parent01".to_string());
    subtask2.status = TaskStatus::Backlog;
    tasks
        .create(&subtask2)
        .await
        .expect("Create subtask2 should succeed");

    let mut subtask3 = make_task("subtsk03", "casclist", "Subtask 3");
    subtask3.parent_id = Some("parent01".to_string());
    subtask3.status = TaskStatus::Backlog;
    tasks
        .create(&subtask3)
        .await
        .expect("Create subtask3 should succeed");

    // Update parent: backlog → in_progress
    parent.status = TaskStatus::InProgress;
    tasks
        .update(&parent)
        .await
        .expect("Update parent should succeed");

    // Verify: All 3 subtasks cascaded to in_progress
    let updated_subtask1 = tasks
        .get("subtsk01")
        .await
        .expect("Get subtask1 should succeed");
    let updated_subtask2 = tasks
        .get("subtsk02")
        .await
        .expect("Get subtask2 should succeed");
    let updated_subtask3 = tasks
        .get("subtsk03")
        .await
        .expect("Get subtask3 should succeed");

    assert_eq!(
        updated_subtask1.status,
        TaskStatus::InProgress,
        "Subtask 1 should cascade to in_progress"
    );
    assert_eq!(
        updated_subtask2.status,
        TaskStatus::InProgress,
        "Subtask 2 should cascade to in_progress"
    );
    assert_eq!(
        updated_subtask3.status,
        TaskStatus::InProgress,
        "Subtask 3 should cascade to in_progress"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cascade_only_matching_subtasks_diverged_unchanged() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("cascmix1", "Mixed Cascade Test"))
        .await
        .expect("Create task list should succeed");

    // Create parent (status: backlog)
    let mut parent = make_task("parent02", "cascmix1", "Parent task");
    parent.status = TaskStatus::Backlog;
    tasks
        .create(&parent)
        .await
        .expect("Create parent should succeed");

    // Create 2 subtasks matching parent status (backlog)
    let mut matching1 = make_task("match001", "cascmix1", "Matching 1");
    matching1.parent_id = Some("parent02".to_string());
    matching1.status = TaskStatus::Backlog;
    tasks
        .create(&matching1)
        .await
        .expect("Create matching1 should succeed");

    let mut matching2 = make_task("match002", "cascmix1", "Matching 2");
    matching2.parent_id = Some("parent02".to_string());
    matching2.status = TaskStatus::Backlog;
    tasks
        .create(&matching2)
        .await
        .expect("Create matching2 should succeed");

    // Create 1 diverged subtask (in_progress, different from parent)
    let mut diverged = make_task("diverg01", "cascmix1", "Diverged");
    diverged.parent_id = Some("parent02".to_string());
    diverged.status = TaskStatus::InProgress;
    tasks
        .create(&diverged)
        .await
        .expect("Create diverged should succeed");

    // Update parent: backlog → done
    parent.status = TaskStatus::Done;
    tasks
        .update(&parent)
        .await
        .expect("Update parent should succeed");

    // Verify: Only matching subtasks cascaded
    let updated_matching1 = tasks
        .get("match001")
        .await
        .expect("Get matching1 should succeed");
    let updated_matching2 = tasks
        .get("match002")
        .await
        .expect("Get matching2 should succeed");
    let updated_diverged = tasks
        .get("diverg01")
        .await
        .expect("Get diverged should succeed");

    assert_eq!(
        updated_matching1.status,
        TaskStatus::Done,
        "Matching subtask 1 should cascade to done"
    );
    assert_eq!(
        updated_matching2.status,
        TaskStatus::Done,
        "Matching subtask 2 should cascade to done"
    );
    assert_eq!(
        updated_diverged.status,
        TaskStatus::InProgress,
        "Diverged subtask should remain in_progress"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cascade_bidirectional_backward() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("cascback", "Backward Cascade Test"))
        .await
        .expect("Create task list should succeed");

    // Create parent + subtasks (all status: done)
    let mut parent = make_task("parent03", "cascback", "Parent done");
    parent.status = TaskStatus::Done;
    tasks
        .create(&parent)
        .await
        .expect("Create parent should succeed");

    let mut subtask1 = make_task("subtsk04", "cascback", "Subtask done 1");
    subtask1.parent_id = Some("parent03".to_string());
    subtask1.status = TaskStatus::Done;
    tasks
        .create(&subtask1)
        .await
        .expect("Create subtask1 should succeed");

    let mut subtask2 = make_task("subtsk05", "cascback", "Subtask done 2");
    subtask2.parent_id = Some("parent03".to_string());
    subtask2.status = TaskStatus::Done;
    tasks
        .create(&subtask2)
        .await
        .expect("Create subtask2 should succeed");

    // Update parent backward: done → in_progress
    parent.status = TaskStatus::InProgress;
    tasks
        .update(&parent)
        .await
        .expect("Update parent should succeed");

    // Verify: Subtasks cascaded backward
    let updated_subtask1 = tasks
        .get("subtsk04")
        .await
        .expect("Get subtask1 should succeed");
    let updated_subtask2 = tasks
        .get("subtsk05")
        .await
        .expect("Get subtask2 should succeed");

    assert_eq!(
        updated_subtask1.status,
        TaskStatus::InProgress,
        "Subtask 1 should cascade backward to in_progress"
    );
    assert_eq!(
        updated_subtask2.status,
        TaskStatus::InProgress,
        "Subtask 2 should cascade backward to in_progress"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cascade_cancelled_status() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("casccanc", "Cancelled Cascade Test"))
        .await
        .expect("Create task list should succeed");

    // Create parent + subtask (both in_progress)
    let mut parent = make_task("parent04", "casccanc", "Parent in progress");
    parent.status = TaskStatus::InProgress;
    tasks
        .create(&parent)
        .await
        .expect("Create parent should succeed");

    let mut subtask = make_task("subtsk06", "casccanc", "Subtask in progress");
    subtask.parent_id = Some("parent04".to_string());
    subtask.status = TaskStatus::InProgress;
    tasks
        .create(&subtask)
        .await
        .expect("Create subtask should succeed");

    // Cancel parent: in_progress → cancelled
    parent.status = TaskStatus::Cancelled;
    tasks
        .update(&parent)
        .await
        .expect("Update parent should succeed");

    // Verify: Subtask cascaded to cancelled
    let updated_subtask = tasks
        .get("subtsk06")
        .await
        .expect("Get subtask should succeed");
    assert_eq!(
        updated_subtask.status,
        TaskStatus::Cancelled,
        "Subtask should cascade to cancelled"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn no_cascade_when_updating_subtask() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("nocasc01", "No Cascade Test"))
        .await
        .expect("Create task list should succeed");

    // Create parent + subtask
    let mut parent = make_task("parent05", "nocasc01", "Parent backlog");
    parent.status = TaskStatus::Backlog;
    tasks
        .create(&parent)
        .await
        .expect("Create parent should succeed");

    let mut subtask = make_task("subtsk07", "nocasc01", "Subtask backlog");
    subtask.parent_id = Some("parent05".to_string());
    subtask.status = TaskStatus::Backlog;
    tasks
        .create(&subtask)
        .await
        .expect("Create subtask should succeed");

    // Update SUBTASK status (not parent)
    subtask.status = TaskStatus::InProgress;
    tasks
        .update(&subtask)
        .await
        .expect("Update subtask should succeed");

    // Verify: Parent unchanged (cascade only works parent → subtasks, not reverse)
    let unchanged_parent = tasks
        .get("parent05")
        .await
        .expect("Get parent should succeed");
    assert_eq!(
        unchanged_parent.status,
        TaskStatus::Backlog,
        "Parent should remain backlog when subtask changes"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cascade_sets_timestamps_correctly() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("cascts01", "Cascade Timestamps Test"))
        .await
        .expect("Create task list should succeed");

    // Create parent + subtask (both backlog, no timestamps)
    let mut parent = make_task("parent06", "cascts01", "Parent backlog");
    parent.status = TaskStatus::Backlog;
    tasks
        .create(&parent)
        .await
        .expect("Create parent should succeed");

    let mut subtask = make_task("subtsk08", "cascts01", "Subtask backlog");
    subtask.parent_id = Some("parent06".to_string());
    subtask.status = TaskStatus::Backlog;
    tasks
        .create(&subtask)
        .await
        .expect("Create subtask should succeed");

    // Verify subtask has no timestamps initially
    let initial_subtask = tasks
        .get("subtsk08")
        .await
        .expect("Get initial subtask should succeed");
    assert!(
        initial_subtask.started_at.is_none(),
        "Subtask should have no started_at initially"
    );
    assert!(
        initial_subtask.completed_at.is_none(),
        "Subtask should have no completed_at initially"
    );

    // Update parent: backlog → in_progress
    parent.status = TaskStatus::InProgress;
    tasks
        .update(&parent)
        .await
        .expect("Update parent to in_progress should succeed");

    // Verify: Subtask started_at was set
    let after_in_progress = tasks
        .get("subtsk08")
        .await
        .expect("Get subtask after in_progress should succeed");
    assert!(
        after_in_progress.started_at.is_some(),
        "Subtask should have started_at after cascade to in_progress"
    );
    assert!(
        after_in_progress.completed_at.is_none(),
        "Subtask should have no completed_at yet"
    );

    // Update parent: in_progress → done
    parent.status = TaskStatus::Done;
    tasks
        .update(&parent)
        .await
        .expect("Update parent to done should succeed");

    // Verify: Subtask completed_at was set
    let after_done = tasks
        .get("subtsk08")
        .await
        .expect("Get subtask after done should succeed");
    assert!(
        after_done.started_at.is_some(),
        "Subtask should still have started_at"
    );
    assert!(
        after_done.completed_at.is_some(),
        "Subtask should have completed_at after cascade to done"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cascade_works_with_standalone_parent() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("cascstnd", "Standalone Parent Test"))
        .await
        .expect("Create task list should succeed");

    // Create standalone parent (no subtasks)
    let mut parent = make_task("parent07", "cascstnd", "Standalone parent");
    parent.status = TaskStatus::Backlog;
    tasks
        .create(&parent)
        .await
        .expect("Create parent should succeed");

    // Update standalone parent
    parent.status = TaskStatus::Done;
    let result = tasks.update(&parent).await;

    // Verify: Update succeeds (no error when no subtasks to cascade)
    assert!(
        result.is_ok(),
        "Standalone parent should update successfully"
    );

    let updated_parent = tasks
        .get("parent07")
        .await
        .expect("Get parent should succeed");
    assert_eq!(
        updated_parent.status,
        TaskStatus::Done,
        "Standalone parent should be done"
    );
}

// ============================================================================
// task_type Filter Tests
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_type_task_returns_only_parents() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("type0001", "Type Filter Test"))
        .await
        .expect("Create task list should succeed");

    // Create 2 parents + 3 subtasks, all done
    let mut parent1 = make_task("partyp01", "type0001", "Parent 1");
    parent1.status = TaskStatus::Done;
    tasks.create(&parent1).await.expect("Create parent1");

    let mut parent2 = make_task("partyp02", "type0001", "Parent 2");
    parent2.status = TaskStatus::Done;
    tasks.create(&parent2).await.expect("Create parent2");

    let mut subtask1 = make_task("subtyp01", "type0001", "Subtask 1");
    subtask1.parent_id = Some("partyp01".to_string());
    subtask1.status = TaskStatus::Done;
    tasks.create(&subtask1).await.expect("Create subtask1");

    let mut subtask2 = make_task("subtyp02", "type0001", "Subtask 2");
    subtask2.parent_id = Some("partyp01".to_string());
    subtask2.status = TaskStatus::Done;
    tasks.create(&subtask2).await.expect("Create subtask2");

    let mut subtask3 = make_task("subtyp03", "type0001", "Subtask 3");
    subtask3.parent_id = Some("partyp02".to_string());
    subtask3.status = TaskStatus::Done;
    tasks.create(&subtask3).await.expect("Create subtask3");

    // Query with type=task
    let query = TaskQuery {
        page: Default::default(),
        list_id: Some("type0001".to_string()),
        parent_id: None,
        status: Some("done".to_string()),
        tags: None,
        task_type: Some("task".to_string()),
    };

    let result = tasks.list(Some(&query)).await.expect("List should succeed");

    // Assert: Only 2 parents returned, no subtasks
    assert_eq!(result.total, 2, "Should return only 2 parent tasks");
    assert_eq!(result.items.len(), 2, "Should have 2 items");

    let ids: Vec<&str> = result.items.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.contains(&"partyp01"), "Should include partyp01");
    assert!(ids.contains(&"partyp02"), "Should include partyp02");
    assert!(!ids.contains(&"subtyp01"), "Should NOT include subtyp01");
    assert!(!ids.contains(&"subtyp02"), "Should NOT include subtyp02");
    assert!(!ids.contains(&"subtyp03"), "Should NOT include subtyp03");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_type_subtask_returns_only_subtasks() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("type0002", "Subtask Filter Test"))
        .await
        .expect("Create task list should succeed");

    // Create 1 parent + 2 subtasks
    let mut parent = make_task("partyp03", "type0002", "Parent");
    parent.status = TaskStatus::Done;
    tasks.create(&parent).await.expect("Create parent");

    let mut subtask1 = make_task("subtyp04", "type0002", "Subtask 1");
    subtask1.parent_id = Some("partyp03".to_string());
    subtask1.status = TaskStatus::Done;
    tasks.create(&subtask1).await.expect("Create subtask1");

    let mut subtask2 = make_task("subtyp05", "type0002", "Subtask 2");
    subtask2.parent_id = Some("partyp03".to_string());
    subtask2.status = TaskStatus::Done;
    tasks.create(&subtask2).await.expect("Create subtask2");

    // Query with type=subtask
    let query = TaskQuery {
        page: Default::default(),
        list_id: Some("type0002".to_string()),
        parent_id: None,
        status: Some("done".to_string()),
        tags: None,
        task_type: Some("subtask".to_string()),
    };

    let result = tasks.list(Some(&query)).await.expect("List should succeed");

    // Assert: Only 2 subtasks returned, no parent
    assert_eq!(result.total, 2, "Should return only 2 subtasks");
    assert_eq!(result.items.len(), 2, "Should have 2 items");

    let ids: Vec<&str> = result.items.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.contains(&"subtyp04"), "Should include subtyp04");
    assert!(ids.contains(&"subtyp05"), "Should include subtyp05");
    assert!(!ids.contains(&"partyp03"), "Should NOT include partyp03");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_type_omitted_returns_all() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("type0003", "Omitted Type Test"))
        .await
        .expect("Create task list should succeed");

    // Create 1 parent + 1 subtask
    let mut parent = make_task("partyp04", "type0003", "Parent");
    parent.status = TaskStatus::Done;
    tasks.create(&parent).await.expect("Create parent");

    let mut subtask = make_task("subtyp06", "type0003", "Subtask");
    subtask.parent_id = Some("partyp04".to_string());
    subtask.status = TaskStatus::Done;
    tasks.create(&subtask).await.expect("Create subtask");

    // Query with type omitted (None)
    let query = TaskQuery {
        page: Default::default(),
        list_id: Some("type0003".to_string()),
        parent_id: None,
        status: Some("done".to_string()),
        tags: None,
        task_type: None,
    };

    let result = tasks.list(Some(&query)).await.expect("List should succeed");

    // Assert: All tasks returned (backward compatibility)
    assert_eq!(
        result.total, 2,
        "Should return all 2 tasks (backward compat)"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_type_works_with_parent_id_filter() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("type0004", "Combined Filter Test"))
        .await
        .expect("Create task list should succeed");

    // Create parent with subtasks
    let mut parent = make_task("partyp05", "type0004", "Parent");
    parent.status = TaskStatus::Done;
    tasks.create(&parent).await.expect("Create parent");

    let mut subtask1 = make_task("subtyp07", "type0004", "Subtask 1");
    subtask1.parent_id = Some("partyp05".to_string());
    subtask1.status = TaskStatus::Done;
    tasks.create(&subtask1).await.expect("Create subtask1");

    let mut subtask2 = make_task("subtyp08", "type0004", "Subtask 2");
    subtask2.parent_id = Some("partyp05".to_string());
    subtask2.status = TaskStatus::Done;
    tasks.create(&subtask2).await.expect("Create subtask2");

    // Query: parent_id=partyp05 AND type=subtask
    // This is valid! We want subtasks of specific parent
    let query = TaskQuery {
        page: Default::default(),
        list_id: Some("type0004".to_string()),
        parent_id: Some("partyp05".to_string()),
        status: Some("done".to_string()),
        tags: None,
        task_type: Some("subtask".to_string()),
    };

    let result = tasks.list(Some(&query)).await.expect("List should succeed");

    // Assert: Both filters applied (parent_id AND type=subtask)
    assert_eq!(
        result.total, 2,
        "Should return 2 subtasks of specific parent"
    );
}

// =============================================================================
// updated_at cascade tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn task_update_cascades_updated_at_to_parent() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    let list = task_lists
        .create(&TaskList {
            id: String::new(),
            title: "Cascade Test".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: None,
        })
        .await
        .expect("Create list");

    // Create parent task with None timestamps so DB generates current time
    let mut parent_task = make_task("parent01", &list.id, "Parent Task");
    parent_task.created_at = None;
    parent_task.updated_at = None;
    let parent = tasks.create(&parent_task).await.unwrap();

    let initial_parent_updated_at = parent.updated_at.clone();

    // Wait to ensure timestamp difference (SQLite second precision)
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Create subtask
    let mut subtask = make_task("subtask1", &list.id, "Subtask");
    subtask.parent_id = Some(parent.id.clone());
    subtask.created_at = None;
    subtask.updated_at = None;
    let subtask = tasks.create(&subtask).await.unwrap();

    // Wait again to ensure update gets different timestamp
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Update the subtask
    let mut updated_subtask = subtask.clone();
    updated_subtask.title = "Updated Subtask".to_string();
    tasks
        .update(&updated_subtask)
        .await
        .expect("Update subtask");

    // Fetch both parent and subtask after update
    let parent_after = tasks.get(&parent.id).await.expect("Get parent");
    let subtask_after = tasks.get(&subtask.id).await.expect("Get subtask");

    // ASSERT: Parent's updated_at should have changed
    assert_ne!(
        parent_after.updated_at, initial_parent_updated_at,
        "Parent's updated_at should be updated when subtask changes"
    );
    // With new trigger logic: parent's updated_at should EQUAL child's updated_at
    assert_eq!(
        parent_after.updated_at, subtask_after.updated_at,
        "Parent's updated_at should equal subtask's updated_at (cascade trigger)"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_create_cascades_updated_at_to_parent() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    let list = task_lists
        .create(&TaskList {
            id: String::new(),
            title: "Cascade Insert Test".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: None,
        })
        .await
        .expect("Create list");

    // Create parent task with None timestamps so DB generates current time
    let mut parent_task = make_task("parent02", &list.id, "Parent Task");
    parent_task.created_at = None;
    parent_task.updated_at = None;
    let parent = tasks.create(&parent_task).await.unwrap();

    let initial_parent_updated_at = parent.updated_at.clone();

    // Wait to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Create subtask - this INSERT should trigger cascade
    let mut subtask = make_task("subtask2", &list.id, "New Subtask");
    subtask.parent_id = Some(parent.id.clone());
    subtask.created_at = None;
    subtask.updated_at = None;
    let created_subtask = tasks.create(&subtask).await.unwrap();

    // Fetch parent again
    let parent_after = tasks.get(&parent.id).await.expect("Get parent");

    // ASSERT: Parent's updated_at should have changed when subtask was created
    assert_ne!(
        parent_after.updated_at, initial_parent_updated_at,
        "Parent's updated_at should be updated when subtask is CREATED (INSERT trigger)"
    );
    assert!(
        parent_after.updated_at >= created_subtask.updated_at,
        "Parent's updated_at should be >= subtask's created_at"
    );
}
