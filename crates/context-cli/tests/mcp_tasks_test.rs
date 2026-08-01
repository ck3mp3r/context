//! Tests for Task MCP tools
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use context_core::{
    HasProjects, HasTaskLists, HasTasks, Project, ProjectRepository, Task, TaskList,
    TaskListRepository, TaskRepository, TaskStatus,
};
use context_db::SqliteDatabase;
use context_server::api::notifier::ChangeNotifier;
use context_server::mcp::tools::tasks::{
    CreateTaskParams, DeleteTaskParams, GetTaskParams, ListTasksParams, TaskTools,
    TransitionTaskParams, UpdateTaskParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ContentBlock;
use std::sync::Arc;

async fn setup_db() -> SqliteDatabase {
    common::setup_db().await
}

async fn create_test_project(db: &SqliteDatabase) -> String {
    let project = Project {
        id: "testproj".to_string(),
        title: "Test Project".to_string(),
        description: Some("Test project for tasks".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.projects().create(&project).await.unwrap();
    project.id
}

async fn create_test_task_list(db: &SqliteDatabase, project_id: &str) -> String {
    let task_list = TaskList {
        id: "testlist".to_string(),
        title: "Test List".to_string(),
        description: Some("Test task list".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: context_core::TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: project_id.to_string(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    };
    db.task_lists().create(&task_list).await.unwrap();
    task_list.id
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_empty() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTasksParams {
        list_id: "testlist".to_string(),
        query: None,
        status: None,
        tags: None,
        parent_id: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_tasks(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = CreateTaskParams {
        list_id: list_id.clone(),
        title: "Test Task".to_string(),
        description: Some("Test description".to_string()),
        priority: Some(3),
        tags: Some(vec!["test".to_string()]),
        parent_id: None,
        external_refs: None,
    };

    let result = tools.create_task(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["title"], "Test Task");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_empty_title() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = CreateTaskParams {
        list_id: list_id.clone(),
        title: "".to_string(),
        description: None,
        priority: None,
        tags: None,
        parent_id: None,
        external_refs: None,
    };

    let result = tools.create_task(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let task = Task {
        id: "task0001".to_string(),
        list_id: list_id.clone(),
        title: "Test Task".to_string(),
        description: Some("Test description".to_string()),
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = GetTaskParams {
        task_id: "task0001".to_string(),
    };

    let result = tools.get_task(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["title"], "Test Task");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = GetTaskParams {
        task_id: "nonexist".to_string(),
    };

    let result = tools.get_task(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let task = Task {
        id: "task0001".to_string(),
        list_id: list_id.clone(),
        title: "Original".to_string(),
        description: Some("Original desc".to_string()),
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = UpdateTaskParams {
        task_id: "task0001".to_string(),
        title: Some("Updated".to_string()),
        description: Some("Updated desc".to_string()),
        priority: Some(1),
        tags: Some(vec!["updated".to_string()]),
        list_id: None,
        parent_id: None,
        external_refs: None,
    };

    let result = tools.update_task(Parameters(params)).await;
    assert!(result.is_ok());

    let updated = db.tasks().get("task0001").await.unwrap();
    assert_eq!(updated.title, "Updated");
    assert_eq!(updated.description, Some("Updated desc".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = UpdateTaskParams {
        task_id: "nonexist".to_string(),
        title: Some("Updated".to_string()),
        description: None,
        priority: None,
        tags: None,
        list_id: None,
        parent_id: None,
        external_refs: None,
    };

    let result = tools.update_task(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let task = Task {
        id: "task0001".to_string(),
        list_id: list_id.clone(),
        title: "To Delete".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = DeleteTaskParams {
        task_id: "task0001".to_string(),
    };

    let result = tools.delete_task(Parameters(params)).await;
    assert!(result.is_ok());

    let result = db.tasks().get("task0001").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = DeleteTaskParams {
        task_id: "nonexist".to_string(),
    };

    let result = tools.delete_task(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_task() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let task = Task {
        id: "task0001".to_string(),
        list_id: list_id.clone(),
        title: "Test Task".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_ids: vec!["task0001".to_string()],
        status: "in_progress".to_string(),
    };

    let result = tools.transition_task(Parameters(params)).await;
    assert!(result.is_ok());

    let updated = db.tasks().get("task0001").await.unwrap();
    assert_eq!(updated.status, TaskStatus::InProgress);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transition_task_invalid() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let task = Task {
        id: "task0001".to_string(),
        list_id: list_id.clone(),
        title: "Test Task".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = TransitionTaskParams {
        task_ids: vec!["task0001".to_string()],
        status: "invalid_status".to_string(),
    };

    let result = tools.transition_task(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_with_data() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let task = Task {
        id: "task0001".to_string(),
        list_id: list_id.clone(),
        title: "Test Task".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec!["test".to_string()],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTasksParams {
        list_id: list_id.clone(),
        query: None,
        status: None,
        tags: None,
        parent_id: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_tasks(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_status_filter() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let task = Task {
        id: "task0001".to_string(),
        list_id: list_id.clone(),
        title: "Test Task".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTasksParams {
        list_id: list_id.clone(),
        query: None,
        status: Some(vec!["todo".to_string()]),
        tags: None,
        parent_id: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_tasks(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_pagination() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    for i in 0..3 {
        let task = Task {
            id: format!("task{:04}", i),
            list_id: list_id.clone(),
            title: format!("Task {}", i),
            description: None,
            status: TaskStatus::Todo,
            priority: Some(3),
            tags: vec![],
            external_refs: vec![],
            parent_id: None,
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        };
        db.tasks().create(&task).await.unwrap();
    }
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTasksParams {
        list_id: list_id.clone(),
        query: None,
        status: None,
        tags: None,
        parent_id: None,
        task_type: None,
        limit: Some(1),
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_tasks(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["items"].as_array().unwrap().len(), 1);
    assert_eq!(response["total"], 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_search() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let task = Task {
        id: "task0001".to_string(),
        list_id: list_id.clone(),
        title: "Rust Backend".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTasksParams {
        list_id: list_id.clone(),
        query: Some("rust".to_string()),
        status: None,
        tags: None,
        parent_id: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_tasks(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_subtask() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let parent_task = Task {
        id: "parent01".to_string(),
        list_id: list_id.clone(),
        title: "Parent Task".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&parent_task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = CreateTaskParams {
        list_id: list_id.clone(),
        title: "Sub Task".to_string(),
        description: None,
        priority: None,
        tags: None,
        parent_id: Some("parent01".to_string()),
        external_refs: None,
    };

    let result = tools.create_task(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["title"], "Sub Task");
    assert_eq!(response["parent_id"], "parent01");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_with_parent_filter() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let parent_task = Task {
        id: "parent01".to_string(),
        list_id: list_id.clone(),
        title: "Parent".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&parent_task).await.unwrap();
    let child_task = Task {
        id: "child001".to_string(),
        list_id: list_id.clone(),
        title: "Child".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: Some("parent01".to_string()),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&child_task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTasksParams {
        list_id: list_id.clone(),
        query: None,
        status: None,
        tags: None,
        parent_id: Some("parent01".to_string()),
        task_type: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_tasks(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
    assert_eq!(response["items"][0]["id"], "child001");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_type_filter() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let parent_task = Task {
        id: "parent01".to_string(),
        list_id: list_id.clone(),
        title: "Parent".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&parent_task).await.unwrap();
    let child_task = Task {
        id: "child001".to_string(),
        list_id: list_id.clone(),
        title: "Child".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        parent_id: Some("parent01".to_string()),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&child_task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTasksParams {
        list_id: list_id.clone(),
        query: None,
        status: None,
        tags: None,
        parent_id: None,
        task_type: Some("task".to_string()),
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_tasks(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
    assert_eq!(response["items"][0]["id"], "parent01");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_sorting() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    for (id, title) in [
        ("task0001", "Charlie"),
        ("task0002", "Alpha"),
        ("task0003", "Bravo"),
    ] {
        let task = Task {
            id: id.to_string(),
            list_id: list_id.clone(),
            title: title.to_string(),
            description: None,
            status: TaskStatus::Todo,
            priority: Some(3),
            tags: vec![],
            external_refs: vec![],
            parent_id: None,
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        };
        db.tasks().create(&task).await.unwrap();
    }
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTasksParams {
        list_id: list_id.clone(),
        query: None,
        status: None,
        tags: None,
        parent_id: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: Some("title".to_string()),
        order: Some("asc".to_string()),
    };

    let result = tools.list_tasks(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let titles: Vec<&str> = response["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Alpha", "Bravo", "Charlie"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_tasks_tag_filter() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let list_id = create_test_task_list(&db, &project_id).await;
    let task = Task {
        id: "task0001".to_string(),
        list_id: list_id.clone(),
        title: "Tagged Task".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec!["backend".to_string(), "rust".to_string()],
        external_refs: vec![],
        parent_id: None,
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.tasks().create(&task).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTasksParams {
        list_id: list_id.clone(),
        query: None,
        status: None,
        tags: Some(vec!["backend".to_string()]),
        parent_id: None,
        task_type: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_tasks(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}
