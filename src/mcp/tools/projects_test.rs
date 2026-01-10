//! Tests for Project MCP tools

use crate::api::notifier::ChangeNotifier;
use crate::db::{Database, Project, ProjectRepository, SqliteDatabase};
use crate::mcp::tools::projects::ProjectTools;
use rmcp::model::{CallToolResult, RawContent};
use serde_json::json;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_empty() {
    // Create isolated database for this test
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use crate::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            limit: None,
            sort: None,
            order: None,
        }))
        .await;
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
    // No default project in migration
    assert_eq!(projects.len(), 0);
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
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };

    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use crate::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            limit: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let projects: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();
    // Only our test project, no default
    assert_eq!(projects.len(), 1);
    assert!(projects.iter().any(|p| p["title"] == "Test Project"));
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
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

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

    let tools = ProjectTools::new(db, ChangeNotifier::new());

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

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use crate::mcp::tools::projects::CreateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .create_project(Parameters(CreateProjectParams {
            title: "New Project".to_string(),
            description: Some("A new project".to_string()),
            tags: None,
            external_refs: None,
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
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    // Update the project
    use crate::mcp::tools::projects::UpdateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .update_project(Parameters(UpdateProjectParams {
            id: "12345678".to_string(),
            title: Some("Updated Title".to_string()),
            description: Some("Updated Description".to_string()),
            tags: None,
            external_refs: None,
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
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

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

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_respects_limit() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create 25 test projects (plus 1 Default from migration = 26 total)
    for i in 0..25 {
        let project = Project {
            id: format!("proj{:04}", i),
            title: format!("Project {}", i),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:00".to_string(),
        };
        db.projects().create(&project).await.unwrap();
    }

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use crate::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;

    // Test 1: Without limit parameter, should return DEFAULT_LIMIT (10)
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            limit: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());
    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let projects: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();
    assert_eq!(projects.len(), 10, "Should return DEFAULT_LIMIT (10) items");

    // Test 2: With limit=5, should return 5
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            limit: Some(5),
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());
    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let projects: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();
    assert_eq!(projects.len(), 5, "Should return requested 5 items");

    // Test 3: With limit=50 (exceeds MAX_LIMIT), should cap at MAX_LIMIT (20)
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            limit: Some(50),
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());
    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let projects: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();
    assert_eq!(
        projects.len(),
        20,
        "Should cap at MAX_LIMIT (20) even though 50 requested"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects_with_sort_and_order() {
    use crate::mcp::tools::projects::ListProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;

    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create projects with specific data for sorting
    // Note: Like Repo, Project's timestamps are auto-generated, so we can't control exact values
    let project1 = Project {
        id: String::new(),
        title: "ZZZ Project".to_string(),
        description: Some("First project".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: String::new(), // Will be auto-generated
        updated_at: String::new(), // Will be auto-generated
    };

    let project2 = Project {
        id: String::new(),
        title: "AAA Project".to_string(),
        description: Some("Second project".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };

    let project3 = Project {
        id: String::new(),
        title: "MMM Project".to_string(),
        description: Some("Third project".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: String::new(),
        updated_at: String::new(),
    };

    db.projects().create(&project1).await.unwrap();
    db.projects().create(&project2).await.unwrap();
    db.projects().create(&project3).await.unwrap();

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    // Test sorting by title ASC
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            limit: None,
            sort: Some("title".to_string()),
            order: Some("asc".to_string()),
        }))
        .await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let projects: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();

    assert_eq!(projects.len(), 3);
    // Should be sorted by title ASC: AAA, MMM, ZZZ
    assert_eq!(projects[0]["title"], "AAA Project");
    assert_eq!(projects[1]["title"], "MMM Project");
    assert_eq!(projects[2]["title"], "ZZZ Project");

    // Test sorting by title DESC
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            limit: None,
            sort: Some("title".to_string()),
            order: Some("desc".to_string()),
        }))
        .await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let projects: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();

    assert_eq!(projects.len(), 3);
    // Should be sorted by title DESC: ZZZ, MMM, AAA
    assert_eq!(projects[0]["title"], "ZZZ Project");
    assert_eq!(projects[1]["title"], "MMM Project");
    assert_eq!(projects[2]["title"], "AAA Project");

    // Test sorting by created_at DESC
    // Note: created_at is auto-generated, so we verify order logic, not exact values
    let result = tools
        .list_projects(Parameters(ListProjectsParams {
            limit: None,
            sort: Some("created_at".to_string()),
            order: Some("desc".to_string()),
        }))
        .await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let projects: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();

    assert_eq!(projects.len(), 3);
    // Verify order is DESC by comparing timestamps
    let ts0 = projects[0]["created_at"].as_str().unwrap();
    let ts1 = projects[1]["created_at"].as_str().unwrap();
    let ts2 = projects[2]["created_at"].as_str().unwrap();
    assert!(ts0 >= ts1, "First timestamp should be >= second");
    assert!(ts1 >= ts2, "Second timestamp should be >= third");
}

// =============================================================================
// External Reference Support
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_external_ref() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    use crate::mcp::tools::projects::CreateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .create_project(Parameters(CreateProjectParams {
            title: "GitHub Linked Project".to_string(),
            description: Some("Project linked to GitHub".to_string()),
            tags: None,
            external_refs: Some(vec!["owner/repo#123".to_string()]),
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
    assert_eq!(project_json["title"], "GitHub Linked Project");
    assert_eq!(project_json["external_refs"], json!(["owner/repo#123"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_project_external_ref() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create initial project without external_ref
    let project = Project {
        id: "12345678".to_string(),
        title: "Project Without Ref".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    let tools = ProjectTools::new(Arc::clone(&db), ChangeNotifier::new());

    // Update to add external_ref
    use crate::mcp::tools::projects::UpdateProjectParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .update_project(Parameters(UpdateProjectParams {
            id: "12345678".to_string(),
            title: Some("Project With Ref".to_string()),
            description: None,
            tags: None,
            external_refs: Some(vec!["JIRA-456".to_string()]),
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
    assert_eq!(project_json["title"], "Project With Ref");
    assert_eq!(project_json["external_refs"], json!(["JIRA-456"]));
}

// =============================================================================
// FTS5 Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_search_projects_by_title() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create test projects
    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Rust Backend API".to_string(),
            description: Some("Web API in Rust".to_string()),
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Python Data Pipeline".to_string(),
            description: Some("Data processing".to_string()),
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use crate::mcp::tools::projects::SearchProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .search_projects(Parameters(SearchProjectsParams {
            query: "rust".to_string(),
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;

    assert!(result.is_ok());
    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let items = response["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Rust Backend API");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_projects_by_description() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create test projects
    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Project Alpha".to_string(),
            description: Some("Machine learning research".to_string()),
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Project Beta".to_string(),
            description: Some("Frontend application".to_string()),
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use crate::mcp::tools::projects::SearchProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .search_projects(Parameters(SearchProjectsParams {
            query: "machine learning".to_string(),
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;

    assert!(result.is_ok());
    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let items = response["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Project Alpha");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_projects_by_tags() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create test projects
    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Frontend App".to_string(),
            description: None,
            tags: vec!["react".to_string(), "typescript".to_string()],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Backend Service".to_string(),
            description: None,
            tags: vec!["rust".to_string(), "api".to_string()],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use crate::mcp::tools::projects::SearchProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .search_projects(Parameters(SearchProjectsParams {
            query: "typescript".to_string(),
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;

    assert!(result.is_ok());
    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let items = response["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Frontend App");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_projects_by_external_refs() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create test projects
    db.projects()
        .create(&Project {
            id: String::new(),
            title: "GitHub Integration".to_string(),
            description: None,
            tags: vec![],
            external_refs: vec!["owner/repo#123".to_string()],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Jira Integration".to_string(),
            description: None,
            tags: vec![],
            external_refs: vec!["PROJ-789".to_string()],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use crate::mcp::tools::projects::SearchProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .search_projects(Parameters(SearchProjectsParams {
            query: "owner/repo#123".to_string(),
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;

    assert!(result.is_ok());
    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let items = response["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "GitHub Integration");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_projects_with_boolean_operators() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create test projects
    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Rust Web API".to_string(),
            description: Some("Backend service".to_string()),
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Rust CLI Tool".to_string(),
            description: Some("Command line".to_string()),
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Python API".to_string(),
            description: Some("Backend service".to_string()),
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use crate::mcp::tools::projects::SearchProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .search_projects(Parameters(SearchProjectsParams {
            query: "rust AND backend".to_string(),
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;

    assert!(result.is_ok());
    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let items = response["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Rust Web API");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_projects_empty_results() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a project
    db.projects()
        .create(&Project {
            id: String::new(),
            title: "Test Project".to_string(),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        })
        .await
        .unwrap();

    let tools = ProjectTools::new(db, ChangeNotifier::new());

    use crate::mcp::tools::projects::SearchProjectsParams;
    use rmcp::handler::server::wrapper::Parameters;
    let result = tools
        .search_projects(Parameters(SearchProjectsParams {
            query: "nonexistent".to_string(),
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;

    assert!(result.is_ok());
    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let items = response["items"].as_array().unwrap();
    assert_eq!(items.len(), 0);
    assert_eq!(response["total"], 0);
}
