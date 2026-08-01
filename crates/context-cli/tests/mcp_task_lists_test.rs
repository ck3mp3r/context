//! Tests for TaskList MCP tools
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use context_core::{
    HasProjects, HasTaskLists, HasTasks, Project, ProjectRepository, Task, TaskList,
    TaskListRepository, TaskListStatus, TaskRepository, TaskStatus,
};
use context_db::SqliteDatabase;
use context_server::api::notifier::ChangeNotifier;
use context_server::mcp::tools::task_lists::{
    CreateTaskListParams, DeleteTaskListParams, GetTaskListParams, GetTaskListStatsParams,
    ListTaskListsParams, TaskListTools, UpdateTaskListParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use std::sync::Arc;

async fn setup_db() -> SqliteDatabase {
    common::setup_db().await
}

async fn create_test_project(db: &SqliteDatabase) -> String {
    let project = Project {
        id: "testproj".to_string(),
        title: "Test Project".to_string(),
        description: Some("Test project".to_string()),
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

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_empty() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTaskListsParams {
        query: None,
        tags: None,
        status: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_task_lists(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_list() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = CreateTaskListParams {
        title: "Test List".to_string(),
        description: Some("Test description".to_string()),
        tags: Some(vec!["test".to_string()]),
        project_id: project_id.clone(),
        notes: None,
        repo_ids: None,
        external_refs: None,
    };

    let result = tools.create_task_list(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["title"], "Test List");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_list_empty_title() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = CreateTaskListParams {
        title: "".to_string(),
        description: None,
        tags: None,
        project_id: project_id.clone(),
        notes: None,
        repo_ids: None,
        external_refs: None,
    };

    let result = tools.create_task_list(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_list() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: Some("Test description".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: project_id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    };
    db.task_lists().create(&task_list).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = GetTaskListParams {
        id: "list0001".to_string(),
    };

    let result = tools.get_task_list(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["title"], "Test List");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_list_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = GetTaskListParams {
        id: "nonexist".to_string(),
    };

    let result = tools.get_task_list(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_list() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Original".to_string(),
        description: Some("Original desc".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: project_id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    };
    db.task_lists().create(&task_list).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = UpdateTaskListParams {
        id: "list0001".to_string(),
        title: "Updated".to_string(),
        description: Some("Updated desc".to_string()),
        tags: Some(vec!["updated".to_string()]),
        status: None,
        notes: None,
        repo_ids: None,
        external_refs: None,
        project_id: None,
    };

    let result = tools.update_task_list(Parameters(params)).await;
    assert!(result.is_ok());

    let updated = db.task_lists().get("list0001").await.unwrap();
    assert_eq!(updated.title, "Updated");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_list_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = UpdateTaskListParams {
        id: "nonexist".to_string(),
        title: "Updated".to_string(),
        description: None,
        tags: None,
        status: None,
        notes: None,
        repo_ids: None,
        external_refs: None,
        project_id: None,
    };

    let result = tools.update_task_list(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task_list() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "To Delete".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: project_id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    };
    db.task_lists().create(&task_list).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = DeleteTaskListParams {
        id: "list0001".to_string(),
    };

    let result = tools.delete_task_list(Parameters(params)).await;
    assert!(result.is_ok());

    let result = db.task_lists().get("list0001").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task_list_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = DeleteTaskListParams {
        id: "nonexist".to_string(),
    };

    let result = tools.delete_task_list(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_list_stats() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Stats List".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: project_id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    };
    db.task_lists().create(&task_list).await.unwrap();

    let task = Task {
        id: "task0001".to_string(),
        list_id: "list0001".to_string(),
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
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = GetTaskListStatsParams {
        id: "list0001".to_string(),
    };

    let result = tools.get_task_list_stats(Parameters(params)).await;
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
async fn test_list_task_lists_with_data() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: None,
        tags: vec!["test".to_string()],
        external_refs: vec![],
        status: TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: project_id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    };
    db.task_lists().create(&task_list).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTaskListsParams {
        query: None,
        tags: None,
        status: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_task_lists(Parameters(params)).await;
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
async fn test_list_task_lists_pagination() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    for i in 0..3 {
        let task_list = TaskList {
            id: format!("list{:04}", i),
            title: format!("List {}", i),
            description: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            notes: None,
            repo_ids: vec![],
            project_id: project_id.clone(),
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
            archived_at: None,
        };
        db.task_lists().create(&task_list).await.unwrap();
    }
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTaskListsParams {
        query: None,
        tags: None,
        status: None,
        project_id: None,
        limit: Some(1),
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_task_lists(Parameters(params)).await;
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
async fn test_list_task_lists_search() {
    let db = setup_db().await;
    let project_id = create_test_project(&db).await;
    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Rust Backend".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: project_id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    };
    db.task_lists().create(&task_list).await.unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTaskListsParams {
        query: Some("rust".to_string()),
        tags: None,
        status: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_task_lists(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}
