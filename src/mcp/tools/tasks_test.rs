//! Tests for Task MCP tools

use crate::api::notifier::ChangeNotifier;
use crate::db::{
    Database, ProjectRepository, SqliteDatabase, Task, TaskList, TaskListRepository,
    TaskRepository, TaskStatus,
};
use crate::mcp::tools::tasks::{
    CreateTaskParams, DeleteTaskParams, GetTaskParams, ListTasksParams, TaskTools,
    TransitionTaskParams, UpdateTaskParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::RawContent;
use std::sync::Arc;

/// Helper to get the default project ID created by migrations
async fn create_test_project(db: &SqliteDatabase) -> String {
    use crate::db::Project;

    let project = Project {
        id: "testproj".to_string(),
        title: "Test Project".to_string(),
        description: Some("Test project for tasks".to_string()),
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };

    db.projects().create(&project).await.unwrap();
    project.id
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_empty() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    // Create a task list first
    let project_id = create_test_project(&db).await;
    let task_list = TaskList {
        id: String::new(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&task_list).await.unwrap();

    let params = ListTasksParams {
        list_id: created_list.id.clone(),
        status: None,
        parent_id: None,
        tags: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_tasks(Parameters(params))
        .await
        .expect("list_tasks should succeed");

    // Parse JSON response
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    // Empty list should have 0 tasks
    assert_eq!(json["total"], 0);
    assert_eq!(json["items"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_and_list_task() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    // Create a task list first
    let task_list = TaskList {
        id: String::new(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: create_test_project(&db).await,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&task_list).await.unwrap();

    // Create task
    let create_params = CreateTaskParams {
        list_id: created_list.id.clone(),
        title: "Implement feature X".to_string(),
        description: None,
        priority: Some(1),
        parent_id: None,
        tags: Some(vec!["urgent".to_string()]),
    };

    let result = tools
        .create_task(Parameters(create_params))
        .await
        .expect("create should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let created: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(created.title, "Implement feature X");
    assert_eq!(created.status, TaskStatus::Backlog);
    assert_eq!(created.priority, Some(1));
    assert_eq!(created.tags, vec!["urgent".to_string()]);
    assert_eq!(created.list_id, created_list.id);
    assert!(created.parent_id.is_none());
    assert!(!created.id.is_empty());

    // List tasks
    let list_params = ListTasksParams {
        list_id: created_list.id.clone(),
        status: None,
        parent_id: None,
        tags: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_tasks(Parameters(list_params))
        .await
        .expect("list should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    assert_eq!(json["items"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create task list and task
    let project_id = create_test_project(&db).await;
    let task_list = TaskList {
        id: String::new(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&task_list).await.unwrap();

    let task = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        title: "Test task for get".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(2),
        tags: vec!["test".to_string()],
        created_at: None,
        started_at: None,
        completed_at: None,
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    let created_task = db.tasks().create(&task).await.unwrap();

    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    // Get the task
    let params = GetTaskParams {
        task_id: created_task.id.clone(),
    };

    let result = tools
        .get_task(Parameters(params))
        .await
        .expect("get should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let retrieved: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(retrieved.id, created_task.id);
    assert_eq!(retrieved.title, "Test task for get");
    assert_eq!(retrieved.status, TaskStatus::Todo);
    assert_eq!(retrieved.priority, Some(2));
    assert_eq!(retrieved.tags, vec!["test".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_not_found() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = GetTaskParams {
        task_id: "nonexist".to_string(),
    };

    let result = tools.get_task(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_filtered_by_status() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a task list
    let task_list = TaskList {
        id: String::new(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: create_test_project(&db).await,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&task_list).await.unwrap();

    // Create tasks with different statuses
    let task1 = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        title: "Task 1".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: None,
        tags: vec![],
        created_at: None,
        started_at: None,
        completed_at: None,
        updated_at: None,
    };
    let task2 = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        title: "Task 2".to_string(),
        description: None,
        status: TaskStatus::Done,
        priority: None,
        tags: vec![],
        created_at: None,
        started_at: None,
        completed_at: Some("2025-12-27T12:00:00Z".to_string()),
        updated_at: None,
    };
    db.tasks().create(&task1).await.unwrap();
    db.tasks().create(&task2).await.unwrap();

    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    // List only "done" tasks
    let params = ListTasksParams {
        list_id: created_list.id.clone(),
        status: Some(vec!["done".to_string()]),
        parent_id: None,
        tags: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_tasks(Parameters(params))
        .await
        .expect("list should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["title"], "Task 2");
    assert_eq!(items[0]["status"], "done");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create task list and task
    let task_list = TaskList {
        id: String::new(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: create_test_project(&db).await,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&task_list).await.unwrap();

    let task = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        title: "Original title".to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: Some(3),
        tags: vec![],
        created_at: None,
        started_at: None,
        completed_at: None,
        updated_at: None,
    };
    let created_task = db.tasks().create(&task).await.unwrap();

    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    // Update task (content only - no status changes)
    let update_params = UpdateTaskParams {
        task_id: created_task.id.clone(),
        title: Some("Updated title".to_string()),
        description: Some("Updated description".to_string()),
        priority: Some(1),
        tags: Some(vec!["urgent".to_string()]),
        parent_id: None,
        list_id: None,
    };

    let result = tools
        .update_task(Parameters(update_params))
        .await
        .expect("update should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.id, created_task.id);
    assert_eq!(updated.title, "Updated title");
    assert_eq!(updated.description, Some("Updated description".to_string()));
    assert_eq!(updated.status, TaskStatus::Backlog); // Status unchanged
    assert_eq!(updated.priority, Some(1));
    assert_eq!(updated.tags, vec!["urgent".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create task list and task
    let task_list = TaskList {
        id: String::new(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: create_test_project(&db).await,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&task_list).await.unwrap();

    let task = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        title: "To be deleted".to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        created_at: None,
        started_at: None,
        completed_at: None,
        updated_at: None,
    };
    let created_task = db.tasks().create(&task).await.unwrap();

    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    // Delete task
    let delete_params = DeleteTaskParams {
        task_id: created_task.id.clone(),
    };

    let result = tools
        .delete_task(Parameters(delete_params))
        .await
        .expect("delete should succeed");

    // Verify success message
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    assert!(content_text.contains("deleted"));

    // Verify task is gone
    let get_result = db.tasks().get(&created_task.id).await;
    assert!(get_result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_with_parent_id_filter() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create task list
    let task_list = TaskList {
        id: String::new(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: create_test_project(&db).await,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&task_list).await.unwrap();

    // Create parent task
    let parent_task = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        title: "Parent task".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: None,
        tags: vec![],
        created_at: None,
        started_at: None,
        completed_at: None,
        updated_at: None,
    };
    let created_parent = db.tasks().create(&parent_task).await.unwrap();

    // Create subtasks
    let subtask = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: Some(created_parent.id.clone()),
        title: "Subtask 1".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: None,
        tags: vec![],
        created_at: None,
        started_at: None,
        completed_at: None,
        updated_at: None,
    };
    db.tasks().create(&subtask).await.unwrap();

    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    // List subtasks only
    let params = ListTasksParams {
        list_id: created_list.id.clone(),
        status: None,
        parent_id: Some(created_parent.id.clone()),
        tags: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_tasks(Parameters(params))
        .await
        .expect("list should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["title"], "Subtask 1");
    assert_eq!(items[0]["parent_id"], created_parent.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_move_to_different_list() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let project_id = create_test_project(&db).await;

    // Create two task lists
    let list1 = TaskList {
        id: String::new(),
        title: "List 1".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: project_id.clone(),
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list1 = db.task_lists().create(&list1).await.unwrap();

    let list2 = TaskList {
        id: String::new(),
        title: "List 2".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list2 = db.task_lists().create(&list2).await.unwrap();

    // Create task in list1
    let task = Task {
        id: String::new(),
        list_id: created_list1.id.clone(),
        parent_id: None,
        title: "Task to move".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec!["move-test".to_string()],
        created_at: None,
        started_at: None,
        completed_at: None,
        updated_at: None,
    };
    let created_task = db.tasks().create(&task).await.unwrap();

    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    // Move task to list2
    let params = UpdateTaskParams {
        task_id: created_task.id.clone(),
        title: None,
        description: None,
        priority: None,
        tags: None,
        parent_id: None,
        list_id: Some(created_list2.id.clone()),
    };

    let result = tools
        .update_task(Parameters(params))
        .await
        .expect("update should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let updated_task: Task = serde_json::from_str(content_text).unwrap();

    // Verify task moved to list2
    assert_eq!(updated_task.list_id, created_list2.id);
    assert_eq!(updated_task.title, "Task to move"); // Title unchanged
    assert_eq!(updated_task.priority, Some(3)); // Priority unchanged
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_parent_id() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a task list
    let task_list = TaskList {
        id: String::new(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: create_test_project(&db).await,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&task_list).await.unwrap();

    // Create a parent task
    let parent_task = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        title: "Parent task".to_string(),
        description: None,
        status: TaskStatus::InProgress,
        priority: Some(2),
        tags: vec![],
        created_at: None,
        started_at: None,
        completed_at: None,
        updated_at: None,
    };
    let created_parent = db.tasks().create(&parent_task).await.unwrap();

    // Create a standalone task (no parent)
    let standalone_task = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        title: "Standalone task".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        created_at: None,
        started_at: None,
        completed_at: None,
        updated_at: None,
    };
    let created_standalone = db.tasks().create(&standalone_task).await.unwrap();

    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    // Update standalone task to become a subtask of parent
    let update_params = UpdateTaskParams {
        task_id: created_standalone.id.clone(),
        title: None,
        description: None,
        priority: None,
        tags: None,
        list_id: None,
        parent_id: Some(created_parent.id.clone()),
    };

    let result = tools
        .update_task(Parameters(update_params))
        .await
        .expect("update should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let updated_task: Task = serde_json::from_str(content_text).unwrap();

    // Verify task is now a subtask of parent
    assert_eq!(updated_task.parent_id, Some(created_parent.id.clone()));
    assert_eq!(updated_task.title, "Standalone task"); // Title unchanged
    assert_eq!(updated_task.priority, Some(3)); // Priority unchanged

    // Test case 2: Remove parent (convert subtask back to standalone)
    let update_params2 = UpdateTaskParams {
        task_id: updated_task.id.clone(),
        title: None,
        description: None,
        priority: None,
        tags: None,
        list_id: None,
        parent_id: Some(String::new()), // Empty string = remove parent
    };

    let result2 = tools
        .update_task(Parameters(update_params2))
        .await
        .expect("update should succeed");

    let content_text2 = match &result2.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let standalone_again: Task = serde_json::from_str(content_text2).unwrap();

    // Verify task is standalone again (no parent)
    assert_eq!(standalone_again.parent_id, None);
    assert_eq!(standalone_again.title, "Standalone task");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_with_sort_and_order() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    // Create project and task list
    let project_id = create_test_project(&db).await;
    let task_list = TaskList {
        id: String::new(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let task_list = db.task_lists().create(&task_list).await.unwrap();

    // Create 3 tasks with controlled timestamps
    let task1 = Task {
        id: String::new(),
        list_id: task_list.id.clone(),
        parent_id: None,
        title: "Alpha Task".to_string(),
        description: None,
        status: TaskStatus::Done,
        priority: Some(1),
        tags: vec![],
        created_at: Some("2025-01-01 10:00:00".to_string()),
        started_at: None,
        completed_at: Some("2025-01-01 11:00:00".to_string()),
        updated_at: Some("2025-01-01 11:00:00".to_string()),
    };
    let task1 = db.tasks().create(&task1).await.unwrap();

    let task2 = Task {
        id: String::new(),
        list_id: task_list.id.clone(),
        parent_id: None,
        title: "Beta Task".to_string(),
        description: None,
        status: TaskStatus::Done,
        priority: Some(2),
        tags: vec![],
        created_at: Some("2025-01-02 10:00:00".to_string()),
        started_at: None,
        completed_at: Some("2025-01-03 11:00:00".to_string()),
        updated_at: Some("2025-01-03 11:00:00".to_string()),
    };
    let task2 = db.tasks().create(&task2).await.unwrap();

    let task3 = Task {
        id: String::new(),
        list_id: task_list.id.clone(),
        parent_id: None,
        title: "Gamma Task".to_string(),
        description: None,
        status: TaskStatus::Done,
        priority: Some(3),
        tags: vec![],
        created_at: Some("2025-01-03 10:00:00".to_string()),
        started_at: None,
        completed_at: Some("2025-01-02 11:00:00".to_string()),
        updated_at: Some("2025-01-02 11:00:00".to_string()),
    };
    let task3 = db.tasks().create(&task3).await.unwrap();

    // Test: Sort by updated_at DESC (newest first)
    let params = ListTasksParams {
        list_id: task_list.id.clone(),
        status: None,
        parent_id: None,
        tags: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: Some("updated_at".to_string()),
        order: Some("desc".to_string()),
    };

    let result = tools
        .list_tasks(Parameters(params))
        .await
        .expect("list_tasks should succeed");

    let response: serde_json::Value =
        serde_json::from_str(&result.content[0].as_text().unwrap().text).unwrap();
    let items = response["items"].as_array().unwrap();

    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["id"], task2.id); // Newest: 2025-01-03
    assert_eq!(items[1]["id"], task3.id); // Middle: 2025-01-02
    assert_eq!(items[2]["id"], task1.id); // Oldest: 2025-01-01
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_with_offset() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a project and task list
    let project_id = create_test_project(&db).await;
    let task_list = db
        .task_lists()
        .create(&TaskList {
            id: String::new(),
            title: "Test List".to_string(),
            description: None,
            notes: None,
            external_ref: None,
            status: crate::db::TaskListStatus::Active,
            tags: vec![],
            project_id,
            repo_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: None,
        })
        .await
        .unwrap();

    // Create 5 tasks
    for i in 1..=5 {
        db.tasks()
            .create(&Task {
                id: String::new(),
                list_id: task_list.id.clone(),
                parent_id: None,
                title: format!("Task {}", i),
                description: None,
                status: TaskStatus::Backlog,
                priority: Some(i),
                tags: vec![],
                created_at: None,
                started_at: None,
                completed_at: None,
                updated_at: None,
            })
            .await
            .unwrap();
    }

    let tools = TaskTools::new(db, ChangeNotifier::new());

    // Test 1: No offset, limit 3 - should get first 3 tasks
    let params = ListTasksParams {
        list_id: task_list.id.clone(),
        status: None,
        parent_id: None,
        tags: None,
        task_type: None,
        limit: Some(3),
        offset: None,
        sort: Some("priority".to_string()),
        order: Some("asc".to_string()),
    };

    let result = tools
        .list_tasks(Parameters(params))
        .await
        .expect("list_tasks should succeed");

    let response: serde_json::Value =
        serde_json::from_str(&result.content[0].as_text().unwrap().text).unwrap();
    let items = response["items"].as_array().unwrap();

    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["title"], "Task 1");
    assert_eq!(items[1]["title"], "Task 2");
    assert_eq!(items[2]["title"], "Task 3");

    // Test 2: Offset 2, limit 3 - should skip first 2 and get next 3
    let params = ListTasksParams {
        list_id: task_list.id.clone(),
        status: None,
        parent_id: None,
        tags: None,
        task_type: None,
        limit: Some(3),
        offset: Some(2),
        sort: Some("priority".to_string()),
        order: Some("asc".to_string()),
    };

    let result = tools
        .list_tasks(Parameters(params))
        .await
        .expect("list_tasks should succeed");

    let response: serde_json::Value =
        serde_json::from_str(&result.content[0].as_text().unwrap().text).unwrap();
    let items = response["items"].as_array().unwrap();

    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["title"], "Task 3");
    assert_eq!(items[1]["title"], "Task 4");
    assert_eq!(items[2]["title"], "Task 5");

    // Test 3: Offset 4, limit 3 - should get only 1 task (Task 5)
    let params = ListTasksParams {
        list_id: task_list.id.clone(),
        status: None,
        parent_id: None,
        tags: None,
        task_type: None,
        limit: Some(3),
        offset: Some(4),
        sort: Some("priority".to_string()),
        order: Some("asc".to_string()),
    };

    let result = tools
        .list_tasks(Parameters(params))
        .await
        .expect("list_tasks should succeed");

    let response: serde_json::Value =
        serde_json::from_str(&result.content[0].as_text().unwrap().text).unwrap();
    let items = response["items"].as_array().unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Task 5");
}

// =============================================================================
// transition_task Tests
// =============================================================================

/// Helper to create a task with specific status for testing
async fn create_task_with_status(
    db: &Arc<SqliteDatabase>,
    status: TaskStatus,
    started_at: Option<String>,
) -> Task {
    let project_id = create_test_project(db).await;
    let task_list = TaskList {
        id: String::new(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&task_list).await.unwrap();

    let task = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        title: "Test Task".to_string(),
        description: None,
        status,
        priority: None,
        tags: vec![],
        created_at: None,
        started_at,
        completed_at: None,
        updated_at: None,
    };
    db.tasks().create(&task).await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_backlog_to_todo() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(&db, TaskStatus::Backlog, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "todo".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Todo);
    assert!(updated.started_at.is_none()); // No timestamp for todo
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_backlog_to_in_progress() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(&db, TaskStatus::Backlog, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "in_progress".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::InProgress);
    assert!(updated.started_at.is_some()); // started_at should be set
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_todo_to_backlog() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(&db, TaskStatus::Todo, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "backlog".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Backlog);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_todo_to_in_progress() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(&db, TaskStatus::Todo, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "in_progress".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::InProgress);
    assert!(updated.started_at.is_some()); // started_at should be set
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_in_progress_to_todo() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::InProgress,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "todo".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Todo);
    assert!(updated.started_at.is_some()); // Preserve started_at when pausing
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_in_progress_to_review() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::InProgress,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "review".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Review);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_in_progress_to_done() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::InProgress,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "done".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Done);
    assert!(updated.completed_at.is_some()); // completed_at should be set
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_in_progress_to_cancelled() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::InProgress,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "cancelled".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Cancelled);
}

// =============================================================================
// transition_task Invalid Transition Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_backlog_to_review_invalid() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(&db, TaskStatus::Backlog, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "review".to_string(),
    };

    let result = tools.transition_task(Parameters(params)).await;

    assert!(result.is_err(), "Backlog → review should be invalid");
    let err = result.unwrap_err();
    assert_eq!(err.code, rmcp::model::ErrorCode(-32602));
    assert_eq!(err.message, "invalid_transition");

    let data = err.data.expect("Error should have data");
    assert_eq!(
        data.get("current_status").and_then(|v| v.as_str()),
        Some("backlog")
    );
    assert_eq!(
        data.get("attempted_status").and_then(|v| v.as_str()),
        Some("review")
    );
    // Verify allowed_statuses is present
    assert!(data.get("allowed_statuses").is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_backlog_to_done_invalid() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(&db, TaskStatus::Backlog, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "done".to_string(),
    };

    let result = tools.transition_task(Parameters(params)).await;

    assert!(result.is_err(), "Backlog → done should be invalid");
    let err = result.unwrap_err();
    assert_eq!(err.message, "invalid_transition");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_todo_to_review_invalid() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(&db, TaskStatus::Todo, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "review".to_string(),
    };

    let result = tools.transition_task(Parameters(params)).await;

    assert!(result.is_err(), "Todo → review should be invalid");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_todo_to_done_invalid() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(&db, TaskStatus::Todo, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "done".to_string(),
    };

    let result = tools.transition_task(Parameters(params)).await;

    assert!(result.is_err(), "Todo → done should be invalid");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_done_to_backlog() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::Done,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "backlog".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Backlog);
    assert!(updated.completed_at.is_none()); // completed_at should be cleared
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_done_to_todo() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::Done,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "todo".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Todo);
    assert!(updated.completed_at.is_none()); // completed_at should be cleared
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_done_to_in_progress() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::Done,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "in_progress".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::InProgress);
    assert!(updated.completed_at.is_none()); // completed_at should be cleared
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_done_to_review() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::Done,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "review".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Review);
    assert!(updated.completed_at.is_none()); // completed_at should be cleared
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_cancelled_to_todo() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(&db, TaskStatus::Cancelled, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "todo".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Todo);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_cancelled_to_in_progress() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(&db, TaskStatus::Cancelled, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "in_progress".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::InProgress);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_review_to_backlog_invalid() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::Review,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "backlog".to_string(),
    };

    let result = tools.transition_task(Parameters(params)).await;

    assert!(result.is_err(), "Review → backlog should be invalid");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_done_to_cancelled_invalid() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::Done,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "cancelled".to_string(),
    };

    let result = tools.transition_task(Parameters(params)).await;

    assert!(result.is_err(), "Done → cancelled should be invalid");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_review_to_in_progress() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::Review,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "in_progress".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::InProgress);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_review_to_done() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::Review,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "done".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Done);
    assert!(updated.completed_at.is_some()); // completed_at should be set
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_review_to_cancelled() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let task = create_task_with_status(
        &db,
        TaskStatus::Review,
        Some("2026-01-01T10:00:00Z".to_string()),
    )
    .await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "cancelled".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Cancelled);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_any_status_to_cancelled() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Test backlog → cancelled
    let task = create_task_with_status(&db, TaskStatus::Backlog, None).await;
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_id: task.id.clone(),
        status: "cancelled".to_string(),
    };

    let result = tools
        .transition_task(Parameters(params))
        .await
        .expect("Transition should succeed");

    let content_text = result.content[0].as_text().unwrap().text.as_str();
    let updated: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.status, TaskStatus::Cancelled);
}
