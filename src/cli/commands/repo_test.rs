use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::PageParams;
use crate::cli::commands::repo::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
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
async fn test_repo_crud_operations() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // CREATE: Repo with all fields populated
    let create_request = CreateRepoRequest {
        remote: "https://github.com/acme/backend-api".to_string(),
        path: Some("/home/dev/projects/backend-api".to_string()),
        tags: vec![
            "backend".to_string(),
            "api".to_string(),
            "production".to_string(),
        ],
        project_ids: vec![],
    };
    let create_result = create_repo(&api_client, create_request).await;
    assert!(create_result.is_ok(), "Should create repo with full data");

    // Extract repo ID
    let output = create_result.unwrap();
    assert!(output.contains("Created repository"));
    let repo_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract repo ID");

    // GET: Verify all fields persisted
    let get_result = get_repo(&api_client, repo_id, "json")
        .await
        .expect("Failed to get repo");
    let fetched_repo: Repo = serde_json::from_str(&get_result).unwrap();

    assert_eq!(fetched_repo.remote, "https://github.com/acme/backend-api");
    assert_eq!(
        fetched_repo.path,
        Some("/home/dev/projects/backend-api".to_string())
    );
    assert_eq!(fetched_repo.tags, vec!["backend", "api", "production"]);

    // UPDATE: Change multiple fields
    let update_request = UpdateRepoRequest {
        remote: Some("https://github.com/acme/backend-api-v2".to_string()),
        path: Some("/home/dev/projects/backend-api-v2".to_string()),
        tags: Some(vec![
            "backend".to_string(),
            "api".to_string(),
            "v2".to_string(),
        ]),
        project_ids: None,
    };
    let update_result = update_repo(&api_client, repo_id, update_request).await;
    assert!(update_result.is_ok(), "Should update repo");

    // Verify updates
    let get_updated = get_repo(&api_client, repo_id, "json")
        .await
        .expect("Failed to get updated repo");
    let updated_repo: Repo = serde_json::from_str(&get_updated).unwrap();

    assert_eq!(
        updated_repo.remote,
        "https://github.com/acme/backend-api-v2"
    );
    assert_eq!(
        updated_repo.path,
        Some("/home/dev/projects/backend-api-v2".to_string())
    );
    assert_eq!(updated_repo.tags, vec!["backend", "api", "v2"]);

    // DELETE: Requires force flag
    let delete_no_force = delete_repo(&api_client, repo_id, false).await;
    assert!(delete_no_force.is_err(), "Should require --force flag");
    assert!(delete_no_force.unwrap_err().to_string().contains("--force"));

    // DELETE: Successful with force
    let delete_result = delete_repo(&api_client, repo_id, true).await;
    assert!(delete_result.is_ok(), "Should delete with --force");

    // Verify deletion
    let get_deleted = get_repo(&api_client, repo_id, "json").await;
    assert!(get_deleted.is_err(), "Should return error for deleted repo");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_repo_list_with_comprehensive_filters() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create diverse repos for filtering
    let repos = vec![
        ("https://github.com/rust/cargo", Some("/path/to/cargo")),
        ("https://github.com/nodejs/node", Some("/path/to/node")),
        ("https://github.com/a/alpha", Some("/path/to/alpha")),
    ];

    for (remote, path) in repos {
        let request = CreateRepoRequest {
            remote: remote.to_string(),
            path: path.map(|p| p.to_string()),
            tags: vec!["development".to_string()],
            project_ids: vec![],
        };
        create_repo(&api_client, request)
            .await
            .expect("Failed to create repo");
    }

    // Test query search
    let result = list_repos(
        &api_client,
        Some("rust"),
        None,
        None,
        PageParams::default(),
        "json",
    )
    .await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
    let repos_found = parsed.as_array().unwrap();
    assert_eq!(repos_found.len(), 1, "Should find 1 repo matching 'rust'");
    assert!(repos_found[0]["remote"].as_str().unwrap().contains("rust"));

    // Test sort ascending
    let page_asc = PageParams {
        limit: None,
        offset: None,
        sort: Some("remote"),
        order: Some("asc"),
    };
    let result_asc = list_repos(&api_client, None, None, None, page_asc, "json").await;
    assert!(result_asc.is_ok());
    let parsed_asc: serde_json::Value = serde_json::from_str(&result_asc.unwrap()).unwrap();
    let repos_asc = parsed_asc.as_array().unwrap();
    assert!(repos_asc[0]["remote"].as_str().unwrap().contains("alpha"));
    assert!(
        repos_asc[repos_asc.len() - 1]["remote"]
            .as_str()
            .unwrap()
            .contains("rust")
    );

    // Test sort descending
    let page_desc = PageParams {
        limit: None,
        offset: None,
        sort: Some("remote"),
        order: Some("desc"),
    };
    let result_desc = list_repos(&api_client, None, None, None, page_desc, "json").await;
    assert!(result_desc.is_ok());
    let parsed_desc: serde_json::Value = serde_json::from_str(&result_desc.unwrap()).unwrap();
    let repos_desc = parsed_desc.as_array().unwrap();
    assert!(repos_desc[0]["remote"].as_str().unwrap().contains("rust"));
    assert!(
        repos_desc[repos_desc.len() - 1]["remote"]
            .as_str()
            .unwrap()
            .contains("alpha")
    );

    // Test offset
    let page_offset = PageParams {
        limit: Some(2),
        offset: Some(1),
        sort: Some("remote"),
        order: Some("asc"),
    };
    let result_offset = list_repos(&api_client, None, None, None, page_offset, "json").await;
    assert!(result_offset.is_ok());
    let parsed_offset: serde_json::Value = serde_json::from_str(&result_offset.unwrap()).unwrap();
    assert_eq!(
        parsed_offset.as_array().unwrap().len(),
        2,
        "Should return 2 repos after skipping 1"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_repo_project_linking() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create projects
    let project1_payload = serde_json::json!({
        "title": "Backend Services Project",
        "description": "Microservices architecture for backend"
    });
    let project1_response = api_client
        .post("/api/v1/projects")
        .json(&project1_payload)
        .send()
        .await
        .expect("Failed to create project 1");
    let project1: serde_json::Value = project1_response.json().await.unwrap();
    let project1_id = project1["id"].as_str().unwrap();

    let project2_payload = serde_json::json!({
        "title": "Frontend Application Project",
        "description": "React-based SPA"
    });
    let project2_response = api_client
        .post("/api/v1/projects")
        .json(&project2_payload)
        .send()
        .await
        .expect("Failed to create project 2");
    let project2: serde_json::Value = project2_response.json().await.unwrap();
    let project2_id = project2["id"].as_str().unwrap();

    // Test 1: Create repo with single project
    let single_project_request = CreateRepoRequest {
        remote: "https://github.com/acme/backend-monorepo".to_string(),
        path: Some("/home/dev/backend".to_string()),
        tags: vec!["backend".to_string(), "monorepo".to_string()],
        project_ids: vec![project1_id.to_string()],
    };
    let create_result = create_repo(&api_client, single_project_request).await;
    assert!(
        create_result.is_ok(),
        "Should create repo with project link"
    );

    let output = create_result.unwrap();
    let repo1_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap();

    let get_repo1 = get_repo(&api_client, repo1_id, "json").await.unwrap();
    let repo1: Repo = serde_json::from_str(&get_repo1).unwrap();
    assert_eq!(repo1.project_ids.len(), 1);
    assert_eq!(repo1.project_ids[0], project1_id);

    // Test 2: Create repo with multiple projects
    let multi_project_request = CreateRepoRequest {
        remote: "https://github.com/acme/shared-components".to_string(),
        path: Some("/home/dev/shared".to_string()),
        tags: vec!["shared".to_string(), "components".to_string()],
        project_ids: vec![project1_id.to_string(), project2_id.to_string()],
    };
    let create_result2 = create_repo(&api_client, multi_project_request).await;
    assert!(
        create_result2.is_ok(),
        "Should create repo with multiple projects"
    );

    let output2 = create_result2.unwrap();
    let repo2_id = output2
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap();

    let get_repo2 = get_repo(&api_client, repo2_id, "json").await.unwrap();
    let repo2: Repo = serde_json::from_str(&get_repo2).unwrap();
    assert_eq!(repo2.project_ids.len(), 2);
    assert!(repo2.project_ids.contains(&project1_id.to_string()));
    assert!(repo2.project_ids.contains(&project2_id.to_string()));

    // Test 3: Update repo to add projects
    let no_project_request = CreateRepoRequest {
        remote: "https://github.com/acme/standalone".to_string(),
        path: None,
        tags: vec!["standalone".to_string()],
        project_ids: vec![],
    };
    let create_result3 = create_repo(&api_client, no_project_request).await;
    assert!(create_result3.is_ok());

    let output3 = create_result3.unwrap();
    let repo3_id = output3
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap();

    // Update to add project link
    let update_request = UpdateRepoRequest {
        remote: None,
        path: None,
        tags: None,
        project_ids: Some(vec![project1_id.to_string()]),
    };
    let update_result = update_repo(&api_client, repo3_id, update_request).await;
    assert!(
        update_result.is_ok(),
        "Should update repo with project link"
    );

    let get_repo3 = get_repo(&api_client, repo3_id, "json").await.unwrap();
    let repo3: Repo = serde_json::from_str(&get_repo3).unwrap();
    assert_eq!(repo3.project_ids.len(), 1);
    assert_eq!(repo3.project_ids[0], project1_id);

    // Test 4: List repos filtered by project_id
    let result = list_repos(
        &api_client,
        None,
        Some(project1_id),
        None,
        PageParams::default(),
        "json",
    )
    .await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
    let repos = parsed.as_array().unwrap();
    // Should find repos 1, 2, and 3 (all linked to project1)
    assert!(
        repos.len() >= 3,
        "Should find at least 3 repos linked to project1"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_repo_error_handling() {
    let (url, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // GET: Non-existent repo
    let get_result = get_repo(&api_client, "nonexist", "json").await;
    assert!(
        get_result.is_err(),
        "Should return error for non-existent repo"
    );

    // UPDATE: Non-existent repo
    let update_request = UpdateRepoRequest {
        remote: Some("https://github.com/test/new".to_string()),
        path: Some("/new/path".to_string()),
        tags: Some(vec!["updated".to_string()]),
        project_ids: None,
    };
    let update_result = update_repo(&api_client, "nonexist", update_request).await;
    assert!(
        update_result.is_err(),
        "Should return error for non-existent repo"
    );
    let error = update_result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );

    // DELETE: Non-existent repo (with force)
    let delete_result = delete_repo(&api_client, "nonexist", true).await;
    assert!(
        delete_result.is_err(),
        "Should return error for non-existent repo"
    );
    let error = delete_result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test]
async fn test_delete_repo_force_flag_validation() {
    // Test the --force flag validation (pure logic, no HTTP needed)
    let api_client = ApiClient::new(None);
    let result = delete_repo(&api_client, "test-id", false).await;

    assert!(result.is_err(), "Should require --force flag");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("--force"),
        "Error should mention --force flag"
    );
}
