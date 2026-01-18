use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::PageParams;
use crate::cli::commands::project::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use serde_json::json;
use tokio::net::TcpListener;

// =============================================================================
// Integration Tests - Consolidated for Coverage with Realistic Data
// =============================================================================

/// Spawn a test HTTP server with in-memory database
async fn spawn_test_server() -> (String, tokio::task::JoinHandle<()>) {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
    );
    let app = routes::create_router(state, false);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (url, handle)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_project_crud_operations() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // CREATE: Project with all fields populated
    let create_request = CreateProjectRequest {
        title: "Backend Microservices Initiative".to_string(),
        description: Some("Migrate monolithic backend to microservices architecture using Kubernetes and service mesh".to_string()),
        tags: Some(vec!["backend".to_string(), "microservices".to_string(), "kubernetes".to_string(), "2026-q1".to_string()]),
        external_refs: Some(vec!["ARCH-2026".to_string(), "github/acme/backend#456".to_string()]),
    };
    let create_result = create_project(&api_client, create_request).await;
    assert!(
        create_result.is_ok(),
        "Should create project with full data"
    );

    // Extract project ID
    let output = create_result.unwrap();
    assert!(output.contains("Created project"));
    let project_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract project ID");

    // GET: Verify all fields persisted
    let get_result = get_project(&api_client, project_id, "json")
        .await
        .expect("Failed to get project");
    let fetched_project: serde_json::Value = serde_json::from_str(&get_result).unwrap();

    assert_eq!(fetched_project["title"], "Backend Microservices Initiative");
    assert_eq!(
        fetched_project["description"],
        "Migrate monolithic backend to microservices architecture using Kubernetes and service mesh"
    );
    assert_eq!(
        fetched_project["tags"],
        json!(["backend", "microservices", "kubernetes", "2026-q1"])
    );
    assert_eq!(
        fetched_project["external_refs"],
        json!(["ARCH-2026", "github/acme/backend#456"])
    );

    // UPDATE: Change multiple fields
    let update_request = UpdateProjectRequest {
        title: Some("Backend Microservices Initiative (Phase 2)".to_string()),
        description: Some("Extended to include observability and monitoring stack".to_string()),
        tags: Some(vec![
            "backend".to_string(),
            "microservices".to_string(),
            "observability".to_string(),
        ]),
        external_refs: Some(vec!["ARCH-2026".to_string(), "MONITOR-789".to_string()]),
    };
    let update_result = update_project(&api_client, project_id, update_request).await;
    assert!(update_result.is_ok(), "Should update project");

    // Verify updates
    let get_updated = get_project(&api_client, project_id, "json")
        .await
        .expect("Failed to get updated project");
    let updated_project: serde_json::Value = serde_json::from_str(&get_updated).unwrap();

    assert_eq!(
        updated_project["title"],
        "Backend Microservices Initiative (Phase 2)"
    );
    assert_eq!(
        updated_project["description"],
        "Extended to include observability and monitoring stack"
    );
    assert_eq!(
        updated_project["tags"],
        json!(["backend", "microservices", "observability"])
    );
    assert_eq!(
        updated_project["external_refs"],
        json!(["ARCH-2026", "MONITOR-789"])
    );

    // DELETE: Requires force flag
    let delete_no_force = delete_project(&api_client, project_id, false).await;
    assert!(delete_no_force.is_err(), "Should require --force flag");
    assert!(delete_no_force.unwrap_err().to_string().contains("--force"));

    // DELETE: Successful with force
    let delete_result = delete_project(&api_client, project_id, true).await;
    assert!(delete_result.is_ok(), "Should delete with --force");

    // Verify deletion
    let get_deleted = get_project(&api_client, project_id, "json").await;
    assert!(
        get_deleted.is_err(),
        "Should return error for deleted project"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_project_list_with_comprehensive_filters() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create diverse projects for filtering
    let projects = vec![
        (
            "Alpha Frontend Redesign",
            "React-based redesign",
            vec!["frontend", "react"],
        ),
        (
            "Beta Data Pipeline",
            "ETL and analytics infrastructure",
            vec!["data", "etl"],
        ),
        (
            "Zebra Mobile App",
            "iOS and Android applications",
            vec!["mobile", "ios"],
        ),
    ];

    for (title, desc, tags) in projects {
        let request = CreateProjectRequest {
            title: title.to_string(),
            description: Some(desc.to_string()),
            tags: Some(tags.iter().map(|s| s.to_string()).collect()),
            external_refs: Some(vec![format!(
                "{}-REF",
                title.split_whitespace().next().unwrap()
            )]),
        };
        create_project(&api_client, request)
            .await
            .expect("Failed to create project");
    }

    // Test empty list (no filters)
    let result_all = list_projects(&api_client, None, None, PageParams::default(), "json").await;
    assert!(result_all.is_ok());
    let parsed_all: serde_json::Value = serde_json::from_str(&result_all.unwrap()).unwrap();
    assert_eq!(parsed_all.as_array().unwrap().len(), 3);

    // Test sort ascending
    let page_asc = PageParams {
        limit: None,
        offset: None,
        sort: Some("title"),
        order: Some("asc"),
    };
    let result_asc = list_projects(&api_client, None, None, page_asc, "json").await;
    assert!(result_asc.is_ok());
    let parsed_asc: serde_json::Value = serde_json::from_str(&result_asc.unwrap()).unwrap();
    let projects_asc = parsed_asc.as_array().unwrap();
    assert_eq!(projects_asc[0]["title"], "Alpha Frontend Redesign");
    assert_eq!(
        projects_asc[projects_asc.len() - 1]["title"],
        "Zebra Mobile App"
    );

    // Test sort descending
    let page_desc = PageParams {
        limit: None,
        offset: None,
        sort: Some("title"),
        order: Some("desc"),
    };
    let result_desc = list_projects(&api_client, None, None, page_desc, "json").await;
    assert!(result_desc.is_ok());
    let parsed_desc: serde_json::Value = serde_json::from_str(&result_desc.unwrap()).unwrap();
    let projects_desc = parsed_desc.as_array().unwrap();
    assert_eq!(projects_desc[0]["title"], "Zebra Mobile App");
    assert_eq!(
        projects_desc[projects_desc.len() - 1]["title"],
        "Alpha Frontend Redesign"
    );

    // Test offset
    let page_offset = PageParams {
        limit: Some(2),
        offset: Some(1),
        sort: Some("title"),
        order: Some("asc"),
    };
    let result_offset = list_projects(&api_client, None, None, page_offset, "json").await;
    assert!(result_offset.is_ok());
    let parsed_offset: serde_json::Value = serde_json::from_str(&result_offset.unwrap()).unwrap();
    assert_eq!(
        parsed_offset.as_array().unwrap().len(),
        2,
        "Should return 2 projects after skipping 1"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_project_error_handling() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // GET: Non-existent project
    let get_result = get_project(&api_client, "nonexist", "json").await;
    assert!(
        get_result.is_err(),
        "Should return error for non-existent project"
    );

    // UPDATE: Non-existent project
    let update_request = UpdateProjectRequest {
        title: Some("New Title".to_string()),
        description: Some("New description".to_string()),
        tags: Some(vec!["updated".to_string()]),
        external_refs: Some(vec!["REF-999".to_string()]),
    };
    let update_result = update_project(&api_client, "nonexist", update_request).await;
    assert!(
        update_result.is_err(),
        "Should return error for non-existent project"
    );
    let error = update_result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );

    // DELETE: Non-existent project (with force)
    let delete_result = delete_project(&api_client, "nonexist", true).await;
    assert!(
        delete_result.is_err(),
        "Should return error for non-existent project"
    );
    let error = delete_result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test]
async fn test_delete_project_force_flag_validation() {
    // Test the --force flag validation (pure logic, no HTTP needed)
    let api_client = ApiClient::new(None);
    let result = delete_project(&api_client, "test-id", false).await;

    assert!(result.is_err(), "Should require --force flag");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("--force"),
        "Error should mention --force flag"
    );
}
