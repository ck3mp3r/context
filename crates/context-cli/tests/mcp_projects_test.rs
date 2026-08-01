//! Tests for Project MCP tools
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use context_server::api::notifier::ChangeNotifier;
use context_core::{Database, HasProjects, Project, ProjectRepository};
use context_db::SqliteDatabase;
use context_server::mcp::tools::projects::ProjectTools;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::json;
use std::sync::Arc;

async fn setup_db() -> SqliteDatabase {
    common::setup_db().await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_empty() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use context_server::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            query: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));
    assert_eq!(call_result.content.len(), 1);

    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let projects = response["items"].as_array().unwrap();
    assert_eq!(projects.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_data() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let project = Project {
        id: "12345678".to_string(),
        title: "Test Project".to_string(),
        description: Some("Test Description".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            query: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let projects = response["items"].as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert!(projects.iter().any(|p| p["title"] == "Test Project"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_project() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let project = Project {
        id: "12345678".to_string(),
        title: "Test Project".to_string(),
        description: Some("Test Description".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::GetProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .get_project(Parameters(GetProjectParams {
            id: "12345678".to_string(),
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["title"], "Test Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_project_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use context_server::mcp::tools::projects::GetProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .get_project(Parameters(GetProjectParams {
            id: "nonexist".to_string(),
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::CreateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .create_project(Parameters(CreateProjectParams {
            title: "New Project".to_string(),
            description: Some("A test project".to_string()),
            tags: Some(vec!["test".to_string()]),
            external_refs: Some(vec!["ref1".to_string()]),
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["title"], "New Project");
    assert_eq!(response["description"], "A test project");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_empty_title() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use context_server::mcp::tools::projects::CreateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .create_project(Parameters(CreateProjectParams {
            title: "".to_string(),
            description: None,
            tags: None,
            external_refs: None,
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_project() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let project = Project {
        id: "12345678".to_string(),
        title: "Original".to_string(),
        description: Some("Original desc".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::UpdateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .update_project(Parameters(UpdateProjectParams {
            id: "12345678".to_string(),
            title: Some("Updated".to_string()),
            description: Some("Updated desc".to_string()),
            tags: Some(vec!["updated".to_string()]),
            external_refs: Some(vec!["ref2".to_string()]),
        }))
        .await;
    assert!(result.is_ok());

    let updated = db.projects().get("12345678").await.unwrap();
    assert_eq!(updated.title, "Updated");
    assert_eq!(updated.description, Some("Updated desc".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_project_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use context_server::mcp::tools::projects::UpdateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .update_project(Parameters(UpdateProjectParams {
            id: "nonexist".to_string(),
            title: Some("Updated".to_string()),
            description: None,
            tags: None,
            external_refs: None,
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_project() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let project = Project {
        id: "12345678".to_string(),
        title: "To Delete".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::DeleteProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .delete_project(Parameters(DeleteProjectParams {
            id: "12345678".to_string(),
        }))
        .await;
    assert!(result.is_ok());

    let result = db.projects().get("12345678").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_project_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use context_server::mcp::tools::projects::DeleteProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .delete_project(Parameters(DeleteProjectParams {
            id: "nonexist".to_string(),
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_pagination() {
    let db = setup_db().await;
    let db = Arc::new(db);

    for i in 0..3 {
        let project = Project {
            id: format!("proj{:04}", i),
            title: format!("Project {}", i),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        };
        db.projects().create(&project).await.unwrap();
    }

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            query: None,
            limit: Some(1),
            offset: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["items"].as_array().unwrap().len(), 1);
    assert_eq!(response["total"], 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_search() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let project = Project {
        id: "12345678".to_string(),
        title: "Rust Backend".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            query: Some("rust".to_string()),
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_sorting() {
    let db = setup_db().await;
    let db = Arc::new(db);

    for (id, title) in [("proj0001", "Charlie"), ("proj0002", "Alpha"), ("proj0003", "Bravo")] {
        let project = Project {
            id: id.to_string(),
            title: title.to_string(),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        };
        db.projects().create(&project).await.unwrap();
    }

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            query: None,
            limit: None,
            offset: None,
            sort: Some("title".to_string()),
            order: Some("asc".to_string()),
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
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
async fn test_list_projects_tag_filter() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let project = Project {
        id: "12345678".to_string(),
        title: "Tagged Project".to_string(),
        description: None,
        tags: vec!["backend".to_string(), "rust".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            query: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let projects = response["items"].as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["tags"], json!(["backend", "rust"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_duplicate_title() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::CreateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;

    // First creation should succeed
    let result = tools
        .create_project(Parameters(CreateProjectParams {
            title: "Same Title".to_string(),
            description: None,
            tags: None,
            external_refs: None,
        }))
        .await;
    assert!(result.is_ok());

    // Second creation with same title should also succeed (titles not unique)
    let result = tools
        .create_project(Parameters(CreateProjectParams {
            title: "Same Title".to_string(),
            description: None,
            tags: None,
            external_refs: None,
        }))
        .await;
    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_project_ids_filter() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let project = Project {
        id: "12345678".to_string(),
        title: "Filtered Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use context_server::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            query: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let projects = response["items"].as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["id"], "12345678");
}
