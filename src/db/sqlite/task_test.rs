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
    db
}

fn make_task_list(id: &str, name: &str) -> TaskList {
    TaskList {
        id: id.to_string(),
        name: name.to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_ref: None,
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
        archived_at: None,
    }
}

fn make_task(id: &str, list_id: &str, content: &str) -> Task {
    Task {
        id: id.to_string(),
        list_id: list_id.to_string(),
        parent_id: None,
        content: content.to_string(),
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        started_at: None,
        completed_at: None,
    }
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
        content: "Complete the implementation".to_string(),
        status: TaskStatus::InProgress,
        priority: Some(2),
        tags: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        started_at: Some("2025-01-02 09:00:00".to_string()),
        completed_at: None,
    };

    tasks.create(&task).await.expect("Create should succeed");

    let retrieved = tasks.get("task0001").await.expect("Get should succeed");
    assert_eq!(retrieved.id, task.id);
    assert_eq!(retrieved.list_id, task.list_id);
    assert_eq!(retrieved.content, task.content);
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

    task.content = "Updated content".to_string();
    task.status = TaskStatus::Done;
    task.completed_at = Some("2025-01-15 17:00:00".to_string());
    task.priority = Some(1);
    tasks.update(&task).await.expect("Update should succeed");

    let retrieved = tasks.get("taskupd1").await.expect("Get should succeed");
    assert_eq!(retrieved.content, "Updated content");
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
        content: "Task with tags".to_string(),
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec!["rust".to_string(), "backend".to_string()],
        created_at: "2025-01-01 00:00:00".to_string(),
        started_at: None,
        completed_at: None,
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
    assert_eq!(results.items[0].content, "Rust task");

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
