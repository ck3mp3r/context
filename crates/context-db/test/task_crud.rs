use crate::SqliteDatabase;
use crate::helpers::{read_deleted_at, read_updated_at};
use context_core::{
    Database, HasTaskLists, HasTasks, Task, TaskList, TaskListRepository, TaskListStatus,
    TaskQuery, TaskRepository, TaskStatus,
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
        updated_at: Some("2025-01-15 11:00:00".to_string()),
        deleted_at: None,
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
        updated_at: None,
        deleted_at: None,
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
        updated_at: Some("2025-01-02 09:00:00".to_string()),
        deleted_at: None,
    };

    tasks.create(&task).await.expect("Create should succeed");

    let retrieved = tasks.get("task0001").await.expect("Get should succeed");
    assert_eq!(retrieved.id, task.id);
    assert_eq!(retrieved.list_id, task.list_id);
    assert_eq!(retrieved.title, task.title);
    assert_eq!(retrieved.status, TaskStatus::InProgress);
    assert_eq!(retrieved.priority, Some(2));
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
    task.priority = Some(1);
    tasks.update(&task).await.expect("Update should succeed");

    let retrieved = tasks.get("taskupd1").await.expect("Get should succeed");
    assert_eq!(retrieved.title, "Updated content");
    assert_eq!(retrieved.status, TaskStatus::Done);
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

    let deleted_at = read_deleted_at(db.pool(), "task", "taskdel1").await;
    assert!(deleted_at.is_some(), "task should be soft-deleted");

    let updated_at = read_updated_at(db.pool(), "task", "taskdel1").await;
    assert_ne!(
        updated_at,
        Some("2025-01-01 00:00:00".to_string()),
        "updated_at should be bumped by the soft-delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_delete_cascades_to_subtasks() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listcasc", "Cascade Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();

    let parent = make_task("tskpar01", "listcasc", "Parent");
    tasks.create(&parent).await.expect("Create parent");

    let mut child = make_task("tskchi01", "listcasc", "Child");
    child.parent_id = Some("tskpar01".to_string());
    tasks.create(&child).await.expect("Create child");

    tasks.delete("tskpar01").await.expect("Delete parent");

    assert!(
        read_deleted_at(db.pool(), "task", "tskpar01")
            .await
            .is_some(),
        "parent task should be soft-deleted"
    );
    assert!(
        read_deleted_at(db.pool(), "task", "tskchi01")
            .await
            .is_some(),
        "child task should be soft-deleted by the cascade"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_delete_terminates_on_cyclic_parent() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listcycl", "Cycle Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();
    let task = make_task("tskcycl1", "listcycl", "Self Parent");
    tasks.create(&task).await.expect("Create task");

    // A self-referencing parent_id is reachable through update: the depth guard
    // only inspects the parent's parent, which is still NULL at this point.
    let mut cyclic = task.clone();
    cyclic.parent_id = Some("tskcycl1".to_string());
    tasks.update(&cyclic).await.expect("Update should succeed");

    // The recursive cascade must terminate instead of looping forever.
    tasks
        .delete("tskcycl1")
        .await
        .expect("Delete should succeed on a cyclic parent chain");

    assert!(
        read_deleted_at(db.pool(), "task", "tskcycl1")
            .await
            .is_some(),
        "task should be soft-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_already_deleted_is_noop() {
    let db = setup_db().await;
    db.task_lists()
        .create(&make_task_list("listnoop", "Noop Test"))
        .await
        .expect("Create task list should succeed");

    let tasks = db.tasks();
    let task = make_task("tsknoop1", "listnoop", "Delete Twice");
    tasks.create(&task).await.expect("Create task");

    tasks.delete("tsknoop1").await.unwrap();
    let first_deleted_at = read_deleted_at(db.pool(), "task", "tsknoop1").await;
    let first_updated_at = read_updated_at(db.pool(), "task", "tsknoop1").await;

    tasks
        .delete("tsknoop1")
        .await
        .expect("Second delete should be a no-op");

    assert_eq!(
        read_deleted_at(db.pool(), "task", "tsknoop1").await,
        first_deleted_at,
        "deleted_at should not change on a repeated delete"
    );
    assert_eq!(
        read_updated_at(db.pool(), "task", "tsknoop1").await,
        first_updated_at,
        "updated_at should not be bumped on a repeated delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_nonexistent_task_returns_not_found() {
    let db = setup_db().await;

    let result = db.tasks().delete("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "deleting a missing task should return NotFound, got: {:?}",
        result
    );
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
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
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
// Task statistics tests
// =============================================================================

// =============================================================================
// Task statistics tests
// =============================================================================

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

// =============================================================================
// Validation Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_task_with_empty_title_should_fail() {
    let db = setup_db().await;
    let tasks = db.tasks();

    // First create a task list
    let list = make_task_list("lst00001", "Test List");
    db.task_lists().create(&list).await.unwrap();

    let task = Task {
        id: "tsk00001".to_string(),
        list_id: "lst00001".to_string(),
        parent_id: None,
        title: "".to_string(), // Empty title
        description: Some("Valid description".to_string()),
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };

    let result = tasks.create(&task).await;
    assert!(result.is_err(), "Create should fail with empty title");

    match result {
        Err(context_core::DbError::Validation { message }) => {
            assert!(
                message.contains("title") && message.contains("empty"),
                "Error should mention empty title, got: {}",
                message
            );
        }
        _ => panic!("Expected DbError::Validation"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn create_task_with_invalid_priority_should_fail() {
    let db = setup_db().await;
    let tasks = db.tasks();

    // First create a task list
    let list = make_task_list("lst00002", "Test List");
    db.task_lists().create(&list).await.unwrap();

    // Test priority too low
    let task = Task {
        id: "tsk00002".to_string(),
        list_id: "lst00002".to_string(),
        parent_id: None,
        title: "Valid Title".to_string(),
        description: Some("Valid description".to_string()),
        status: TaskStatus::Todo,
        priority: Some(0), // Invalid - too low
        tags: vec![],
        external_refs: vec![],
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };

    let result = tasks.create(&task).await;
    assert!(result.is_err(), "Create should fail with priority 0");

    // Test priority too high
    let task2 = Task {
        priority: Some(6), // Invalid - too high
        ..task
    };

    let result2 = tasks.create(&task2).await;
    assert!(result2.is_err(), "Create should fail with priority 6");
}

#[tokio::test(flavor = "multi_thread")]
async fn update_task_with_empty_title_should_fail() {
    let db = setup_db().await;
    let tasks = db.tasks();

    // First create a task list
    let list = make_task_list("lst00003", "Test List");
    db.task_lists().create(&list).await.unwrap();

    // Create a valid task
    let task = Task {
        id: "tsk00003".to_string(),
        list_id: "lst00003".to_string(),
        parent_id: None,
        title: "Valid Title".to_string(),
        description: Some("Valid description".to_string()),
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    tasks.create(&task).await.unwrap();

    // Try to update with empty title
    let mut updated = task.clone();
    updated.title = "".to_string();

    let result = tasks.update(&updated).await;
    assert!(result.is_err(), "Update should fail with empty title");
}
