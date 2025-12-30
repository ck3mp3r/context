//! Tests for Task MCP tools

use crate::db::{
    Database, ProjectRepository, SqliteDatabase, Task, TaskList, TaskListRepository,
    TaskRepository, TaskStatus,
};
use crate::mcp::tools::tasks::{
    CompleteTaskParams, CreateTaskParams, DeleteTaskParams, GetTaskParams, ListTasksParams,
    TaskTools, UpdateTaskParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::RawContent;
use std::sync::Arc;

/// Helper to get the default project ID created by migrations
async fn get_default_project_id(db: &SqliteDatabase) -> String {
    let projects = db.projects().list(None).await.unwrap();
    projects
        .items
        .iter()
        .find(|p| p.title == "Default")
        .expect("Default project should exist")
        .id
        .clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_empty() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone());

    // Create a task list first
    let default_project_id = get_default_project_id(&db).await;
    let task_list = TaskList {
        id: String::new(),
        name: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: default_project_id,
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
    let tools = TaskTools::new(db.clone());

    // Create a task list first
    let task_list = TaskList {
        id: String::new(),
        name: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: get_default_project_id(&db).await,
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&task_list).await.unwrap();

    // Create task
    let create_params = CreateTaskParams {
        list_id: created_list.id.clone(),
        content: "Implement feature X".to_string(),
        status: Some("todo".to_string()),
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

    assert_eq!(created.content, "Implement feature X");
    assert_eq!(created.status, TaskStatus::Todo);
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
    let default_project_id = get_default_project_id(&db).await;
    let task_list = TaskList {
        id: String::new(),
        name: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: default_project_id,
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
        content: "Test task for get".to_string(),
        status: TaskStatus::Todo,
        priority: Some(2),
        tags: vec!["test".to_string()],
        created_at: String::new(),
        started_at: None,
        completed_at: None,
    };
    let created_task = db.tasks().create(&task).await.unwrap();

    let tools = TaskTools::new(db.clone());

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
    assert_eq!(retrieved.content, "Test task for get");
    assert_eq!(retrieved.status, TaskStatus::Todo);
    assert_eq!(retrieved.priority, Some(2));
    assert_eq!(retrieved.tags, vec!["test".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_not_found() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone());

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
        name: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: get_default_project_id(&db).await,
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
        content: "Task 1".to_string(),
        status: TaskStatus::Todo,
        priority: None,
        tags: vec![],
        created_at: String::new(),
        started_at: None,
        completed_at: None,
    };
    let task2 = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        content: "Task 2".to_string(),
        status: TaskStatus::Done,
        priority: None,
        tags: vec![],
        created_at: String::new(),
        started_at: None,
        completed_at: Some("2025-12-27T12:00:00Z".to_string()),
    };
    db.tasks().create(&task1).await.unwrap();
    db.tasks().create(&task2).await.unwrap();

    let tools = TaskTools::new(db.clone());

    // List only "done" tasks
    let params = ListTasksParams {
        list_id: created_list.id.clone(),
        status: Some(vec!["done".to_string()]),
        parent_id: None,
        tags: None,
        task_type: None,
        limit: None,
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
    assert_eq!(items[0]["content"], "Task 2");
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
        name: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: get_default_project_id(&db).await,
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
        content: "Original content".to_string(),
        status: TaskStatus::Backlog,
        priority: Some(3),
        tags: vec![],
        created_at: String::new(),
        started_at: None,
        completed_at: None,
    };
    let created_task = db.tasks().create(&task).await.unwrap();

    let tools = TaskTools::new(db.clone());

    // Update task
    let update_params = UpdateTaskParams {
        task_id: created_task.id.clone(),
        content: Some("Updated content".to_string()),
        status: Some("in_progress".to_string()),
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
    assert_eq!(updated.content, "Updated content");
    assert_eq!(updated.status, TaskStatus::InProgress);
    assert_eq!(updated.priority, Some(1));
    assert_eq!(updated.tags, vec!["urgent".to_string()]);
    assert!(updated.started_at.is_some()); // Status change to in_progress sets started_at
}

#[tokio::test(flavor = "multi_thread")]
async fn test_complete_task() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create task list and task
    let task_list = TaskList {
        id: String::new(),
        name: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: get_default_project_id(&db).await,
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
        content: "Complete this task".to_string(),
        status: TaskStatus::InProgress,
        priority: None,
        tags: vec![],
        created_at: String::new(),
        started_at: Some("2025-12-27T10:00:00Z".to_string()),
        completed_at: None,
    };
    let created_task = db.tasks().create(&task).await.unwrap();

    let tools = TaskTools::new(db.clone());

    // Complete task
    let complete_params = CompleteTaskParams {
        task_id: created_task.id.clone(),
    };

    let result = tools
        .complete_task(Parameters(complete_params))
        .await
        .expect("complete should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let completed: Task = serde_json::from_str(content_text).unwrap();

    assert_eq!(completed.id, created_task.id);
    assert_eq!(completed.status, TaskStatus::Done);
    assert!(completed.completed_at.is_some()); // complete_task sets completed_at
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create task list and task
    let task_list = TaskList {
        id: String::new(),
        name: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: get_default_project_id(&db).await,
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
        content: "To be deleted".to_string(),
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        created_at: String::new(),
        started_at: None,
        completed_at: None,
    };
    let created_task = db.tasks().create(&task).await.unwrap();

    let tools = TaskTools::new(db.clone());

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
        name: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: get_default_project_id(&db).await,
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
        content: "Parent task".to_string(),
        status: TaskStatus::Todo,
        priority: None,
        tags: vec![],
        created_at: String::new(),
        started_at: None,
        completed_at: None,
    };
    let created_parent = db.tasks().create(&parent_task).await.unwrap();

    // Create subtasks
    let subtask = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: Some(created_parent.id.clone()),
        content: "Subtask 1".to_string(),
        status: TaskStatus::Todo,
        priority: None,
        tags: vec![],
        created_at: String::new(),
        started_at: None,
        completed_at: None,
    };
    db.tasks().create(&subtask).await.unwrap();

    let tools = TaskTools::new(db.clone());

    // List subtasks only
    let params = ListTasksParams {
        list_id: created_list.id.clone(),
        status: None,
        parent_id: Some(created_parent.id.clone()),
        tags: None,
        task_type: None,
        limit: None,
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
    assert_eq!(items[0]["content"], "Subtask 1");
    assert_eq!(items[0]["parent_id"], created_parent.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_move_to_different_list() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let default_project_id = get_default_project_id(&db).await;

    // Create two task lists
    let list1 = TaskList {
        id: String::new(),
        name: "List 1".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: default_project_id.clone(),
        repo_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list1 = db.task_lists().create(&list1).await.unwrap();

    let list2 = TaskList {
        id: String::new(),
        name: "List 2".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: default_project_id,
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
        content: "Task to move".to_string(),
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec!["move-test".to_string()],
        created_at: String::new(),
        started_at: None,
        completed_at: None,
    };
    let created_task = db.tasks().create(&task).await.unwrap();

    let tools = TaskTools::new(db.clone());

    // Move task to list2
    let params = UpdateTaskParams {
        task_id: created_task.id.clone(),
        content: None,
        status: None,
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
    assert_eq!(updated_task.content, "Task to move"); // Content unchanged
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
        name: "Test List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        status: crate::db::TaskListStatus::Active,
        external_ref: None,
        project_id: get_default_project_id(&db).await,
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
        content: "Parent task".to_string(),
        status: TaskStatus::InProgress,
        priority: Some(2),
        tags: vec![],
        created_at: String::new(),
        started_at: None,
        completed_at: None,
    };
    let created_parent = db.tasks().create(&parent_task).await.unwrap();

    // Create a standalone task (no parent)
    let standalone_task = Task {
        id: String::new(),
        list_id: created_list.id.clone(),
        parent_id: None,
        content: "Standalone task".to_string(),
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        created_at: String::new(),
        started_at: None,
        completed_at: None,
    };
    let created_standalone = db.tasks().create(&standalone_task).await.unwrap();

    let tools = TaskTools::new(db.clone());

    // Update standalone task to become a subtask of parent
    let update_params = UpdateTaskParams {
        task_id: created_standalone.id.clone(),
        content: None,
        status: None,
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
    assert_eq!(updated_task.content, "Standalone task"); // Content unchanged
    assert_eq!(updated_task.priority, Some(3)); // Priority unchanged

    // Test case 2: Remove parent (convert subtask back to standalone)
    let update_params2 = UpdateTaskParams {
        task_id: updated_task.id.clone(),
        content: None,
        status: None,
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
    assert_eq!(standalone_again.content, "Standalone task");
}
