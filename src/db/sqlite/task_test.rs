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
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: "test0000".to_string(), // Test project (created by setup_db)
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
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
        external_refs: vec![],
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
        external_refs: vec![],
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
        external_refs: vec![],
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
        external_refs: vec![],
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
        external_refs: vec![],
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
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: None,
            updated_at: None,
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
            external_refs: vec![],
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
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: None,
            updated_at: None,
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
            external_refs: vec![],
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
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: None,
            updated_at: None,
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
            external_refs: vec![],
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
async fn task_update_preserves_historical_timestamps() {
    let db = setup_db().await;

    let task_lists = db.task_lists();
    let list = task_lists
        .create(&TaskList {
            id: String::new(),
            title: "Timestamp Preservation Test".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: None,
            updated_at: None,
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    let tasks = db.tasks();

    // Test scenarios: (from_status, to_status, started_at_preserved, completed_at_preserved)
    let scenarios = vec![
        // Transitions from Done should preserve completed_at
        (TaskStatus::Done, TaskStatus::Backlog, true, true),
        (TaskStatus::Done, TaskStatus::Todo, true, true),
        (TaskStatus::Done, TaskStatus::InProgress, true, true),
        (TaskStatus::Done, TaskStatus::Review, true, true),
        // Transitions from InProgress should preserve started_at
        (TaskStatus::InProgress, TaskStatus::Backlog, true, false),
        (TaskStatus::InProgress, TaskStatus::Todo, true, false),
    ];

    for (from_status, to_status, expect_started, expect_completed) in scenarios {
        // Create task with appropriate timestamps
        let created = tasks
            .create(&Task {
                id: String::new(),
                list_id: list.id.clone(),
                parent_id: None,
                title: format!("{:?} to {:?}", from_status, to_status),
                description: None,
                status: from_status.clone(),
                priority: None,
                tags: vec![],
                external_refs: vec![],
                created_at: None,
                started_at: if matches!(from_status, TaskStatus::InProgress | TaskStatus::Done) {
                    Some("2025-01-01 10:00:00".to_string())
                } else {
                    None
                },
                completed_at: if matches!(from_status, TaskStatus::Done) {
                    Some("2025-01-01 12:00:00".to_string())
                } else {
                    None
                },
                updated_at: Some("2025-01-01 12:00:00".to_string()),
            })
            .await
            .expect("Create should succeed");

        // Transition to new status
        let mut updated = created.clone();
        updated.status = to_status.clone();
        tasks.update(&updated).await.expect("Update should succeed");

        // Verify timestamps preserved
        let after = tasks.get(&created.id).await.expect("Get should succeed");
        assert_eq!(after.status, to_status);

        if expect_started {
            assert!(
                after.started_at.is_some(),
                "{:?} → {:?}: started_at should be preserved",
                from_status,
                to_status
            );
        }

        if expect_completed {
            assert!(
                after.completed_at.is_some(),
                "{:?} → {:?}: completed_at should be preserved",
                from_status,
                to_status
            );
        }
    }
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
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: None,
            updated_at: None,
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
            external_refs: vec![],
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
async fn task_create_with_github_external_ref() {
    let db = setup_db().await;
    let list = db
        .task_lists()
        .create(&make_task_list("list0001", "Test List"))
        .await
        .expect("Create list");

    let mut task = make_task("task0001", &list.id, "Task with GitHub issue");
    task.external_refs = vec!["ck3mp3r/context#42".to_string()];

    let created = db.tasks().create(&task).await.expect("Create task");

    assert_eq!(
        created.external_refs,
        vec!["ck3mp3r/context#42".to_string()]
    );

    // Verify it's persisted
    let fetched = db.tasks().get(&created.id).await.expect("Get task");
    assert_eq!(
        fetched.external_refs,
        vec!["ck3mp3r/context#42".to_string()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_create_with_jira_external_ref() {
    let db = setup_db().await;
    let list = db
        .task_lists()
        .create(&make_task_list("list0001", "Test List"))
        .await
        .expect("Create list");

    let mut task = make_task("task0001", &list.id, "Task with Jira ticket");
    task.external_refs = vec!["BACKEND-456".to_string()];

    let created = db.tasks().create(&task).await.expect("Create task");

    assert_eq!(created.external_refs, vec!["BACKEND-456".to_string()]);

    // Verify it's persisted
    let fetched = db.tasks().get(&created.id).await.expect("Get task");
    assert_eq!(fetched.external_refs, vec!["BACKEND-456".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn task_update_external_ref() {
    let db = setup_db().await;
    let list = db
        .task_lists()
        .create(&make_task_list("list0001", "Test List"))
        .await
        .expect("Create list");

    // Create task without external_refs
    let task = make_task("task0001", &list.id, "Task");
    let created = db.tasks().create(&task).await.expect("Create task");
    assert_eq!(created.external_refs, Vec::<String>::new());

    // Update to add external_refs
    let mut updated_task = created.clone();
    updated_task.external_refs = vec!["ck3mp3r/context#123".to_string()];
    db.tasks().update(&updated_task).await.expect("Update task");
    let fetched = db.tasks().get(&updated_task.id).await.expect("Get task");
    assert_eq!(
        fetched.external_refs,
        vec!["ck3mp3r/context#123".to_string()]
    );

    // Update to change external_refs
    updated_task.external_refs = vec!["PROJ-789".to_string()];
    db.tasks().update(&updated_task).await.expect("Update task");
    let fetched2 = db.tasks().get(&updated_task.id).await.expect("Get task");
    assert_eq!(fetched2.external_refs, vec!["PROJ-789".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn task_remove_external_ref() {
    let db = setup_db().await;
    let list = db
        .task_lists()
        .create(&make_task_list("list0001", "Test List"))
        .await
        .expect("Create list");

    // Create task with external_refs
    let mut task = make_task("task0001", &list.id, "Task");
    task.external_refs = vec!["ck3mp3r/context#42".to_string()];
    let created = db.tasks().create(&task).await.expect("Create task");
    assert_eq!(
        created.external_refs,
        vec!["ck3mp3r/context#42".to_string()]
    );

    // Remove external_refs by setting to empty vec
    let mut updated_task = created.clone();
    updated_task.external_refs = vec![];
    db.tasks().update(&updated_task).await.expect("Update task");

    // Verify it's persisted
    let fetched = db.tasks().get(&updated_task.id).await.expect("Get task");
    assert_eq!(fetched.external_refs, Vec::<String>::new());
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
        page: crate::db::PageSort {
            limit: Some(10),
            offset: Some(0),
            sort_by: Some("updated_at".to_string()),
            sort_order: Some(crate::db::SortOrder::Desc),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:01".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:01".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:01".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:01".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:01".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:02".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:01".to_string()),
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
        started_at: Some("2025-01-01 10:00:00".to_string()),
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
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
        started_at: Some("2025-01-01 09:00:00".to_string()),
        completed_at: Some("2025-01-01 11:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
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
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
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

// =============================================================================
// Bulk Task Transition Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_success() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup: Create task list
    task_lists
        .create(&make_task_list("bulklist", "Bulk Test"))
        .await
        .expect("Create task list should succeed");

    // Create 3 tasks all with status backlog
    let mut task1 = make_task("bulktsk1", "bulklist", "Task 1");
    task1.status = TaskStatus::Backlog;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    let mut task2 = make_task("bulktsk2", "bulklist", "Task 2");
    task2.status = TaskStatus::Backlog;
    tasks
        .create(&task2)
        .await
        .expect("Create task2 should succeed");

    let mut task3 = make_task("bulktsk3", "bulklist", "Task 3");
    task3.status = TaskStatus::Backlog;
    tasks
        .create(&task3)
        .await
        .expect("Create task3 should succeed");

    // Transition all 3 tasks from backlog to in_progress
    let task_ids = vec![
        "bulktsk1".to_string(),
        "bulktsk2".to_string(),
        "bulktsk3".to_string(),
    ];
    let updated_tasks = tasks
        .transition_tasks(&task_ids, TaskStatus::InProgress)
        .await
        .expect("Bulk transition should succeed");

    // Verify all tasks were updated
    assert_eq!(updated_tasks.len(), 3);
    for task in updated_tasks {
        assert_eq!(task.status, TaskStatus::InProgress);
        assert!(task.started_at.is_some(), "started_at should be set");
        assert!(task.completed_at.is_none(), "completed_at should be None");
    }

    // Verify via get
    let task1_after = tasks
        .get("bulktsk1")
        .await
        .expect("Get task1 should succeed");
    let task2_after = tasks
        .get("bulktsk2")
        .await
        .expect("Get task2 should succeed");
    let task3_after = tasks
        .get("bulktsk3")
        .await
        .expect("Get task3 should succeed");

    assert_eq!(task1_after.status, TaskStatus::InProgress);
    assert_eq!(task2_after.status, TaskStatus::InProgress);
    assert_eq!(task3_after.status, TaskStatus::InProgress);
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_mixed_status_fails() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("mixlist1", "Mixed Status Test"))
        .await
        .expect("Create task list should succeed");

    // Create tasks with different statuses
    let mut task1 = make_task("mixtsk01", "mixlist1", "Task 1");
    task1.status = TaskStatus::Backlog;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    let mut task2 = make_task("mixtsk02", "mixlist1", "Task 2");
    task2.status = TaskStatus::Todo;
    tasks
        .create(&task2)
        .await
        .expect("Create task2 should succeed");

    // Try to transition tasks with different statuses
    let task_ids = vec!["mixtsk01".to_string(), "mixtsk02".to_string()];
    let result = tasks
        .transition_tasks(&task_ids, TaskStatus::InProgress)
        .await;

    assert!(
        result.is_err(),
        "Should fail when tasks have different statuses"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("same current status") || err_msg.contains("mixed status"),
        "Error should mention status mismatch"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_invalid_transition_fails() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("invlist1", "Invalid Transition Test"))
        .await
        .expect("Create task list should succeed");

    // Create tasks with backlog status
    let mut task1 = make_task("invtsk01", "invlist1", "Task 1");
    task1.status = TaskStatus::Backlog;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    let mut task2 = make_task("invtsk02", "invlist1", "Task 2");
    task2.status = TaskStatus::Backlog;
    tasks
        .create(&task2)
        .await
        .expect("Create task2 should succeed");

    // Try invalid transition: backlog -> review (not allowed)
    let task_ids = vec!["invtsk01".to_string(), "invtsk02".to_string()];
    let result = tasks.transition_tasks(&task_ids, TaskStatus::Review).await;

    assert!(result.is_err(), "Should fail for invalid transition");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("invalid_transition") || err_msg.contains("not allowed"),
        "Error should mention invalid transition"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_not_found_fails() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("notflist", "Not Found Test"))
        .await
        .expect("Create task list should succeed");

    // Create only one task
    let mut task1 = make_task("notftsk1", "notflist", "Task 1");
    task1.status = TaskStatus::Backlog;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    // Try to transition with one non-existent task ID
    let task_ids = vec!["notftsk1".to_string(), "nonexist".to_string()];
    let result = tasks.transition_tasks(&task_ids, TaskStatus::Todo).await;

    assert!(result.is_err(), "Should fail when task not found");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not found") || err_msg.contains("NotFound"),
        "Error should mention task not found"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_timestamps() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("tslist01", "Timestamp Test"))
        .await
        .expect("Create task list should succeed");

    // Create tasks
    let mut task1 = make_task("tstsk001", "tslist01", "Task 1");
    task1.status = TaskStatus::Todo;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    let mut task2 = make_task("tstsk002", "tslist01", "Task 2");
    task2.status = TaskStatus::Todo;
    tasks
        .create(&task2)
        .await
        .expect("Create task2 should succeed");

    // Test 1: Transition to in_progress sets started_at
    let task_ids = vec!["tstsk001".to_string(), "tstsk002".to_string()];
    let updated = tasks
        .transition_tasks(&task_ids, TaskStatus::InProgress)
        .await
        .expect("Transition to in_progress should succeed");

    for task in &updated {
        assert!(task.started_at.is_some(), "started_at should be set");
        assert!(task.completed_at.is_none(), "completed_at should be None");
    }

    // Test 2: Transition to done sets completed_at
    let updated = tasks
        .transition_tasks(&task_ids, TaskStatus::Done)
        .await
        .expect("Transition to done should succeed");

    for task in &updated {
        assert!(task.started_at.is_some(), "started_at should remain set");
        assert!(task.completed_at.is_some(), "completed_at should be set");
    }

    // Test 3: Transition back to todo preserves timestamps (audit trail)
    let updated = tasks
        .transition_tasks(&task_ids, TaskStatus::Todo)
        .await
        .expect("Transition to todo should succeed");

    for task in &updated {
        assert!(task.started_at.is_some(), "started_at should be preserved");
        assert!(
            task.completed_at.is_some(),
            "completed_at should be preserved (historical record)"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn transition_tasks_transaction_rollback() {
    let db = setup_db().await;
    let task_lists = db.task_lists();
    let tasks = db.tasks();

    // Setup
    task_lists
        .create(&make_task_list("rolllist", "Rollback Test"))
        .await
        .expect("Create task list should succeed");

    // Create tasks
    let mut task1 = make_task("rolltsk1", "rolllist", "Task 1");
    task1.status = TaskStatus::Backlog;
    tasks
        .create(&task1)
        .await
        .expect("Create task1 should succeed");

    let mut task2 = make_task("rolltsk2", "rolllist", "Task 2");
    task2.status = TaskStatus::Backlog;
    tasks
        .create(&task2)
        .await
        .expect("Create task2 should succeed");

    // Try bulk transition with one non-existent task - should rollback
    let task_ids = vec![
        "rolltsk1".to_string(),
        "rolltsk2".to_string(),
        "nonexist".to_string(),
    ];
    let result = tasks.transition_tasks(&task_ids, TaskStatus::Todo).await;

    assert!(result.is_err(), "Should fail due to non-existent task");

    // Verify no tasks were updated (transaction rolled back)
    let task1_after = tasks
        .get("rolltsk1")
        .await
        .expect("Get task1 should succeed");
    let task2_after = tasks
        .get("rolltsk2")
        .await
        .expect("Get task2 should succeed");

    assert_eq!(
        task1_after.status,
        TaskStatus::Backlog,
        "Task1 should remain backlog"
    );
    assert_eq!(
        task2_after.status,
        TaskStatus::Backlog,
        "Task2 should remain backlog"
    );
}
