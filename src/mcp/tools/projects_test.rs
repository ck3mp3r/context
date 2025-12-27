//! Tests for Project MCP tools

use crate::db::{Database, Project, ProjectRepository, SqliteDatabase};
use crate::mcp::tools::projects::ProjectTools;
use rmcp::model::{CallToolResult, RawContent};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_empty() {
    // Create isolated database for this test
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let tools = ProjectTools::new(db);

    let result = tools.list_projects().await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));
    assert_eq!(call_result.content.len(), 1);

    // Parse the JSON content
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let projects: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();
    // Migration creates a "Default" project
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["title"], "Default");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_data() {
    // Create isolated database for this test
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a test project
    let project = Project {
        id: "12345678".to_string(), // Must be exactly 8 characters
        title: "Test Project".to_string(),
        description: Some("Test Description".to_string()),
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };

    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db));

    let result = tools.list_projects().await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let projects: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();
    // Migration creates a "Default" project, plus our test project = 2
    assert_eq!(projects.len(), 2);
    assert!(projects.iter().any(|p| p["title"] == "Test Project"));
    assert!(projects.iter().any(|p| p["title"] == "Default"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_project() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a test project
    let project = Project {
        id: "12345678".to_string(),
        title: "Test Project".to_string(),
        description: Some("Test Description".to_string()),
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db));

    // Test getting the project
    use crate::mcp::tools::projects::GetProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .get_project(Parameters(GetProjectParams {
            id: "12345678".to_string(),
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let project_json: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(project_json["id"], "12345678");
    assert_eq!(project_json["title"], "Test Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_project_not_found() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let tools = ProjectTools::new(db);

    use crate::mcp::tools::projects::GetProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .get_project(Parameters(GetProjectParams {
            id: "notfound".to_string(),
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let tools = ProjectTools::new(Arc::clone(&db));

    use crate::mcp::tools::projects::CreateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .create_project(Parameters(CreateProjectParams {
            title: "New Project".to_string(),
            description: Some("A new project".to_string()),
            tags: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let project_json: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(project_json["title"], "New Project");
    assert_eq!(project_json["description"], "A new project");
    assert!(project_json["id"].as_str().unwrap().len() == 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_project() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create initial project
    let project = Project {
        id: "12345678".to_string(),
        title: "Original Title".to_string(),
        description: Some("Original Description".to_string()),
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db));

    // Update the project
    use crate::mcp::tools::projects::UpdateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .update_project(Parameters(UpdateProjectParams {
            id: "12345678".to_string(),
            title: Some("Updated Title".to_string()),
            description: Some("Updated Description".to_string()),
            tags: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let project_json: serde_json::Value = serde_json::from_str(content_text).unwrap();
    println!(
        "Project JSON: {}",
        serde_json::to_string_pretty(&project_json).unwrap()
    );
    assert_eq!(project_json["title"], "Updated Title");
    assert_eq!(project_json["description"], "Updated Description");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_project() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a project to delete
    let project = Project {
        id: "12345678".to_string(),
        title: "To Delete".to_string(),
        description: None,
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db));

    // Delete the project
    use crate::mcp::tools::projects::DeleteProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .delete_project(Parameters(DeleteProjectParams {
            id: "12345678".to_string(),
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    // Verify it's deleted
    let get_result = db.projects().get("12345678").await;
    assert!(get_result.is_err());
}
