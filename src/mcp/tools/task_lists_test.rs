//! Tests for TaskList MCP tools

use crate::api::notifier::ChangeNotifier;
use crate::db::{
    Database, Project, ProjectRepository, SqliteDatabase, Task, TaskList, TaskListRepository,
    TaskListStatus, TaskRepository, TaskStatus,
};
use crate::mcp::tools::task_lists::{
    CreateTaskListParams, DeleteTaskListParams, GetTaskListParams, GetTaskListStatsParams,
    ListTaskListsParams, TaskListTools, UpdateTaskListParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::RawContent;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_empty() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = ListTaskListsParams {
        tags: None,
        status: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_task_lists(Parameters(params))
        .await
        .expect("list_task_lists should succeed");

    // Parse JSON response
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    // Empty database should have 0 task lists
    assert_eq!(json["total"], 0);
    assert_eq!(json["items"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_and_get_task_list() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    // Create a project first (required for task list)
    let project = Project {
        id: String::new(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    let created_project = db.projects().create(&project).await.unwrap();

    // Create task list
    let create_params = CreateTaskListParams {
        title: "Sprint 1".to_string(),
        description: Some("First sprint".to_string()),
        notes: Some("Planning notes".to_string()),
        tags: Some(vec!["work".to_string()]),
        external_ref: Some("JIRA-123".to_string()),
        repo_ids: None,
        project_id: created_project.id.clone(),
    };

    let result = tools
        .create_task_list(Parameters(create_params))
        .await
        .expect("create should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let created: TaskList = serde_json::from_str(content_text).unwrap();

    assert_eq!(created.title, "Sprint 1");
    assert_eq!(created.description, Some("First sprint".to_string()));
    assert_eq!(created.notes, Some("Planning notes".to_string()));
    assert_eq!(created.tags, vec!["work".to_string()]);
    assert_eq!(created.external_ref, Some("JIRA-123".to_string()));
    assert_eq!(created.status, TaskListStatus::Active);
    assert_eq!(created.project_id, created_project.id);
    assert!(created.archived_at.is_none());
    assert!(!created.id.is_empty());

    // Get the task list
    let get_params = GetTaskListParams {
        id: created.id.clone(),
    };

    let result = tools
        .get_task_list(Parameters(get_params))
        .await
        .expect("get should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let fetched: TaskList = serde_json::from_str(content_text).unwrap();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.title, "Sprint 1");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_task_list() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    // Create project
    let project = Project {
        id: String::new(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    let created_project = db.projects().create(&project).await.unwrap();

    // Create task list
    let list = TaskList {
        id: String::new(),
        title: "Old Name".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_ref: None,
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: created_project.id.clone(),
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created = db.task_lists().create(&list).await.unwrap();

    // Update task list
    let update_params = UpdateTaskListParams {
        id: created.id.clone(),
        title: "New Name".to_string(),
        description: Some("Updated description".to_string()),
        notes: Some("Updated notes".to_string()),
        tags: Some(vec!["updated".to_string()]),
        external_ref: Some("JIRA-456".to_string()),
        status: Some("archived".to_string()),
        repo_ids: None,
        project_id: None,
    };

    let result = tools
        .update_task_list(Parameters(update_params))
        .await
        .expect("update should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let updated: TaskList = serde_json::from_str(content_text).unwrap();

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.title, "New Name");
    assert_eq!(updated.description, Some("Updated description".to_string()));
    assert_eq!(updated.notes, Some("Updated notes".to_string()));
    assert_eq!(updated.tags, vec!["updated".to_string()]);
    assert_eq!(updated.external_ref, Some("JIRA-456".to_string()));
    assert_eq!(updated.status, TaskListStatus::Archived);
    assert!(updated.archived_at.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_task_list() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    // Create project
    let project = Project {
        id: String::new(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    let created_project = db.projects().create(&project).await.unwrap();

    // Create task list
    let list = TaskList {
        id: String::new(),
        title: "To Delete".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_ref: None,
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: created_project.id.clone(),
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created = db.task_lists().create(&list).await.unwrap();

    // Delete
    let delete_params = DeleteTaskListParams {
        id: created.id.clone(),
    };

    let result = tools
        .delete_task_list(Parameters(delete_params))
        .await
        .expect("delete should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(json["success"], true);

    // Verify deleted
    let get_result = db.task_lists().get(&created.id).await;
    assert!(get_result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_task_lists_with_filters() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    // Create project
    let project = Project {
        id: String::new(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    let created_project = db.projects().create(&project).await.unwrap();

    // Create multiple task lists with different tags and statuses
    let list1 = TaskList {
        id: String::new(),
        title: "List 1".to_string(),
        description: None,
        notes: None,
        tags: vec!["work".to_string()],
        external_ref: None,
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: created_project.id.clone(),
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    db.task_lists().create(&list1).await.unwrap();

    let list2 = TaskList {
        id: String::new(),
        title: "List 2".to_string(),
        description: None,
        notes: None,
        tags: vec!["personal".to_string()],
        external_ref: None,
        status: TaskListStatus::Archived,
        repo_ids: vec![],
        project_id: created_project.id.clone(),
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: Some("2024-01-01T00:00:00Z".to_string()),
    };
    db.task_lists().create(&list2).await.unwrap();

    // Filter by status=active
    let params = ListTaskListsParams {
        tags: None,
        status: Some("active".to_string()),
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_task_lists(Parameters(params))
        .await
        .expect("list should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    assert_eq!(json["items"][0]["title"], "List 1");

    // Filter by tags=personal
    let params = ListTaskListsParams {
        tags: Some("personal".to_string()),
        status: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_task_lists(Parameters(params))
        .await
        .expect("list should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    assert_eq!(json["items"][0]["title"], "List 2");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_nonexistent_task_list() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    let params = GetTaskListParams {
        id: "nonexistent".to_string(),
    };

    let result = tools.get_task_list(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_task_list_stats() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = TaskListTools::new(db.clone(), ChangeNotifier::new());

    // Create project
    let project = Project {
        id: String::new(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };
    let created_project = db.projects().create(&project).await.unwrap();

    // Create task list
    let list = TaskList {
        id: String::new(),
        title: "Stats Test".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_ref: None,
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: created_project.id.clone(),
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let created_list = db.task_lists().create(&list).await.unwrap();

    // Create tasks with different statuses
    for status in [
        TaskStatus::Backlog,
        TaskStatus::Todo,
        TaskStatus::Todo,
        TaskStatus::InProgress,
        TaskStatus::Done,
        TaskStatus::Done,
        TaskStatus::Done,
    ] {
        let task = Task {
            id: String::new(),
            list_id: created_list.id.clone(),
            parent_id: None,
            title: format!("Task with status {:?}", status),
            description: None,
            status,
            priority: None,
            tags: vec![],
            created_at: None,
            started_at: None,
            completed_at: None,
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        };
        db.tasks().create(&task).await.unwrap();
    }

    // Get stats
    let params = GetTaskListStatsParams {
        id: created_list.id.clone(),
    };

    let result = tools
        .get_task_list_stats(Parameters(params))
        .await
        .expect("get_task_list_stats should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["list_id"], created_list.id);
    assert_eq!(json["total"], 7);
    assert_eq!(json["backlog"], 1);
    assert_eq!(json["todo"], 2);
    assert_eq!(json["in_progress"], 1);
    assert_eq!(json["review"], 0);
    assert_eq!(json["done"], 3);
    assert_eq!(json["cancelled"], 0);
}
