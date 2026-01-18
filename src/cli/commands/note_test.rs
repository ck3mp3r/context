use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::PageParams;
use crate::cli::commands::note::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use serde_json::json;
use tokio::net::TcpListener;

// =============================================================================
// Integration Tests - Consolidated for Coverage with Realistic Data
// =============================================================================

/// Spawn a test HTTP server with in-memory database
async fn spawn_test_server() -> (String, String, tokio::task::JoinHandle<()>) {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

    // Create test project
    let project_id = sqlx::query_scalar::<_, String>(
        "INSERT INTO project (id, title, description, tags, created_at, updated_at) 
         VALUES ('test0000', 'Test Project', 'Test project for CLI tests', '[]', datetime('now'), datetime('now')) 
         RETURNING id"
    )
    .fetch_one(db.pool())
    .await
    .expect("Failed to create test project");

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

    (url, project_id, handle)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_note_crud_operations() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // CREATE: Note with all fields populated
    let create_request = CreateNoteRequest {
        title: "Rust Async Programming Guide".to_string(),
        content: "Comprehensive guide covering tokio, async/await, futures, and concurrency patterns in Rust".to_string(),
        tags: Some(vec!["rust".to_string(), "async".to_string(), "programming".to_string(), "guide".to_string()]),
        parent_id: None,
        idx: Some(1),
        project_ids: Some(vec![project_id.clone()]),
        repo_ids: None,
    };
    let create_result = create_note(&api_client, create_request).await;
    assert!(create_result.is_ok(), "Should create note with full data");

    // Extract note ID
    let output = create_result.unwrap();
    let note_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract note ID");

    // GET: Verify all fields persisted
    let get_result = get_note(&api_client, note_id, "json")
        .await
        .expect("Failed to get note");
    let fetched_note: serde_json::Value = serde_json::from_str(&get_result).unwrap();

    assert_eq!(fetched_note["title"], "Rust Async Programming Guide");
    assert_eq!(
        fetched_note["content"],
        "Comprehensive guide covering tokio, async/await, futures, and concurrency patterns in Rust"
    );
    assert_eq!(
        fetched_note["tags"],
        json!(["rust", "async", "programming", "guide"])
    );
    assert_eq!(fetched_note["project_ids"], json!([project_id]));
    assert_eq!(fetched_note["idx"], 1);

    // UPDATE: Change multiple fields
    let update_request = UpdateNoteRequest {
        title: Some("Rust Async Programming Guide (Updated)".to_string()),
        content: Some("Updated guide with additional chapters on streams and channels".to_string()),
        tags: Some(vec![
            "rust".to_string(),
            "async".to_string(),
            "advanced".to_string(),
        ]),
        parent_id: None,
        idx: Some(Some(2)),
        project_ids: Some(vec![project_id.clone()]),
        repo_ids: None,
    };
    let update_result = update_note(&api_client, note_id, update_request).await;
    assert!(update_result.is_ok(), "Should update note");

    // Verify updates
    let get_updated = get_note(&api_client, note_id, "json")
        .await
        .expect("Failed to get updated note");
    let updated_note: serde_json::Value = serde_json::from_str(&get_updated).unwrap();

    assert_eq!(
        updated_note["title"],
        "Rust Async Programming Guide (Updated)"
    );
    assert_eq!(
        updated_note["content"],
        "Updated guide with additional chapters on streams and channels"
    );
    assert_eq!(updated_note["tags"], json!(["rust", "async", "advanced"]));
    assert_eq!(updated_note["idx"], 2);

    // DELETE: Requires force flag
    let delete_no_force = delete_note(&api_client, note_id, false).await;
    assert!(delete_no_force.is_err(), "Should require --force flag");
    assert!(delete_no_force.unwrap_err().to_string().contains("--force"));

    // DELETE: Successful with force
    let delete_result = delete_note(&api_client, note_id, true).await;
    assert!(delete_result.is_ok(), "Should delete with --force");

    // Verify deletion
    let get_deleted = get_note(&api_client, note_id, "json").await;
    assert!(get_deleted.is_err(), "Should return error for deleted note");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_note_list_with_comprehensive_filters() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create diverse notes for filtering
    let notes = vec![
        (
            "Alpha Rust Notes",
            "Rust content",
            vec!["rust", "programming"],
        ),
        (
            "Beta Testing Notes",
            "Testing content",
            vec!["testing", "qa"],
        ),
        (
            "Zebra DevOps Notes",
            "DevOps content",
            vec!["devops", "infrastructure"],
        ),
    ];

    for (title, content, tags) in notes {
        let request = CreateNoteRequest {
            title: title.to_string(),
            content: content.to_string(),
            tags: Some(tags.iter().map(|s| s.to_string()).collect()),
            parent_id: None,
            idx: None,
            project_ids: None,
            repo_ids: None,
        };
        create_note(&api_client, request)
            .await
            .expect("Failed to create note");
    }

    // Test filter by tags
    let result = list_notes(
        &api_client,
        None,
        None,
        Some("rust"),
        None,
        None,
        PageParams::default(),
        "json",
    )
    .await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
    let rust_note = parsed
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["title"] == "Alpha Rust Notes");
    assert!(rust_note.is_some(), "Should find Rust note");

    // Test sort ordering (asc)
    let page_asc = PageParams {
        limit: None,
        offset: None,
        sort: Some("title"),
        order: Some("asc"),
    };
    let result_asc = list_notes(&api_client, None, None, None, None, None, page_asc, "json").await;
    assert!(result_asc.is_ok());
    let parsed_asc: serde_json::Value = serde_json::from_str(&result_asc.unwrap()).unwrap();
    let notes_asc = parsed_asc.as_array().unwrap();
    assert_eq!(notes_asc[0]["title"], "Alpha Rust Notes");
    assert_eq!(
        notes_asc[notes_asc.len() - 1]["title"],
        "Zebra DevOps Notes"
    );

    // Test sort ordering (desc)
    let page_desc = PageParams {
        limit: None,
        offset: None,
        sort: Some("title"),
        order: Some("desc"),
    };
    let result_desc =
        list_notes(&api_client, None, None, None, None, None, page_desc, "json").await;
    assert!(result_desc.is_ok());
    let parsed_desc: serde_json::Value = serde_json::from_str(&result_desc.unwrap()).unwrap();
    let notes_desc = parsed_desc.as_array().unwrap();
    assert_eq!(notes_desc[0]["title"], "Zebra DevOps Notes");
    assert_eq!(
        notes_desc[notes_desc.len() - 1]["title"],
        "Alpha Rust Notes"
    );

    // Test offset parameter
    let page_offset = PageParams {
        limit: Some(2),
        offset: Some(1),
        sort: Some("title"),
        order: Some("asc"),
    };
    let result_offset = list_notes(
        &api_client,
        None,
        None,
        None,
        None,
        None,
        page_offset,
        "json",
    )
    .await;
    assert!(result_offset.is_ok());
    let parsed_offset: serde_json::Value = serde_json::from_str(&result_offset.unwrap()).unwrap();
    assert_eq!(
        parsed_offset.as_array().unwrap().len(),
        2,
        "Should return 2 notes after skipping 1"
    );

    // Test nonexistent tag filter (should not error)
    let result_no_tag = list_notes(
        &api_client,
        None,
        None,
        Some("nonexistent"),
        None,
        None,
        PageParams::default(),
        "json",
    )
    .await;
    assert!(
        result_no_tag.is_ok(),
        "Filtering by nonexistent tag should succeed"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_note_hierarchical_structure() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create parent note with full data
    let parent_request = CreateNoteRequest {
        title: "Project Architecture Document".to_string(),
        content: "High-level overview of system architecture and design decisions".to_string(),
        tags: Some(vec![
            "architecture".to_string(),
            "documentation".to_string(),
        ]),
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    let parent_result = create_note(&api_client, parent_request)
        .await
        .expect("Failed to create parent");

    let parent_id = parent_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract parent ID");

    // Create multiple child notes with different idx values to test ordering
    let child1_request = CreateNoteRequest {
        title: "Backend Services".to_string(),
        content: "Details about microservices architecture and API design".to_string(),
        tags: Some(vec!["backend".to_string(), "api".to_string()]),
        parent_id: Some(parent_id.to_string()),
        idx: Some(2),
        project_ids: None,
        repo_ids: None,
    };
    create_note(&api_client, child1_request)
        .await
        .expect("Failed to create child 1");

    let child2_request = CreateNoteRequest {
        title: "Frontend Application".to_string(),
        content: "React-based SPA with TypeScript and state management".to_string(),
        tags: Some(vec!["frontend".to_string(), "react".to_string()]),
        parent_id: Some(parent_id.to_string()),
        idx: Some(1),
        project_ids: None,
        repo_ids: None,
    };
    let child2_result = create_note(&api_client, child2_request)
        .await
        .expect("Failed to create child 2");

    let child2_id = child2_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract child2 ID");

    // Verify child note has parent_id and idx
    let get_child = get_note(&api_client, child2_id, "json")
        .await
        .expect("Failed to get child");
    let child_note: serde_json::Value = serde_json::from_str(&get_child).unwrap();
    assert_eq!(child_note["parent_id"], parent_id);
    assert_eq!(child_note["idx"], 1);

    // List notes filtered by parent_id - should be ordered by idx
    let result = list_notes(
        &api_client,
        None,
        None,
        None,
        Some(parent_id),
        None,
        PageParams::default(),
        "json",
    )
    .await;
    assert!(result.is_ok());

    let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
    let children = parsed.as_array().unwrap();
    assert_eq!(children.len(), 2);
    // Verify ordering by idx: Frontend (idx=1) before Backend (idx=2)
    assert_eq!(children[0]["title"], "Frontend Application");
    assert_eq!(children[0]["idx"], 1);
    assert_eq!(children[1]["title"], "Backend Services");
    assert_eq!(children[1]["idx"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_note_project_and_repo_linking() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create second project
    let project2_payload = serde_json::json!({"title": "Second Project", "description": "Additional project for testing"});
    let project2_response = api_client
        .post("/api/v1/projects")
        .json(&project2_payload)
        .send()
        .await
        .expect("Failed to create project 2");
    let project2: serde_json::Value = project2_response.json().await.unwrap();
    let project2_id = project2["id"].as_str().unwrap();

    // Create repository
    let repo_payload = serde_json::json!({
        "remote": "https://github.com/acme/notes-example",
        "tags": ["documentation"]
    });
    let repo_response = api_client
        .post("/api/v1/repos")
        .json(&repo_payload)
        .send()
        .await
        .expect("Failed to create repo");
    let repo: serde_json::Value = repo_response.json().await.unwrap();
    let repo_id = repo["id"].as_str().unwrap();

    // Test 1: Note with multiple projects
    let multi_project_request = CreateNoteRequest {
        title: "Cross-Project Architecture Notes".to_string(),
        content: "Shared architecture decisions affecting multiple projects".to_string(),
        tags: Some(vec![
            "architecture".to_string(),
            "multi-project".to_string(),
        ]),
        parent_id: None,
        idx: None,
        project_ids: Some(vec![project_id.clone(), project2_id.to_string()]),
        repo_ids: None,
    };
    let create_result = create_note(&api_client, multi_project_request).await;
    assert!(
        create_result.is_ok(),
        "Should create note with multiple projects"
    );

    let output = create_result.unwrap();
    let note1_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap();
    let get_note1 = get_note(&api_client, note1_id, "json").await.unwrap();
    let note1: serde_json::Value = serde_json::from_str(&get_note1).unwrap();
    let project_ids_val = note1["project_ids"].as_array().unwrap();
    assert_eq!(project_ids_val.len(), 2);
    assert!(project_ids_val.contains(&json!(project_id)));
    assert!(project_ids_val.contains(&json!(project2_id)));

    // Test 2: Note with repo link
    let repo_link_request = CreateNoteRequest {
        title: "Repository Documentation".to_string(),
        content: "Setup and contribution guidelines for the repository".to_string(),
        tags: Some(vec!["docs".to_string(), "contribution".to_string()]),
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: Some(vec![repo_id.to_string()]),
    };
    let create_result2 = create_note(&api_client, repo_link_request).await;
    assert!(create_result2.is_ok(), "Should create note with repo link");

    let output2 = create_result2.unwrap();
    let note2_id = output2
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap();
    let get_note2 = get_note(&api_client, note2_id, "json").await.unwrap();
    let note2: serde_json::Value = serde_json::from_str(&get_note2).unwrap();
    let repo_ids_val = note2["repo_ids"].as_array().unwrap();
    assert_eq!(repo_ids_val.len(), 1);
    assert_eq!(repo_ids_val[0], json!(repo_id));

    // Test 3: Note with BOTH project and repo links
    let combo_request = CreateNoteRequest {
        title: "Implementation Notes".to_string(),
        content: "Detailed implementation notes linking project and repository".to_string(),
        tags: Some(vec!["implementation".to_string(), "technical".to_string()]),
        parent_id: None,
        idx: None,
        project_ids: Some(vec![project_id.clone()]),
        repo_ids: Some(vec![repo_id.to_string()]),
    };
    let create_result3 = create_note(&api_client, combo_request).await;
    assert!(create_result3.is_ok(), "Should create note with both links");

    let output3 = create_result3.unwrap();
    let note3_id = output3
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap();
    let get_note3 = get_note(&api_client, note3_id, "json").await.unwrap();
    let note3: serde_json::Value = serde_json::from_str(&get_note3).unwrap();
    assert_eq!(note3["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(note3["project_ids"][0], json!(project_id));
    assert_eq!(note3["repo_ids"].as_array().unwrap().len(), 1);
    assert_eq!(note3["repo_ids"][0], json!(repo_id));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_note_error_handling() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // GET: Non-existent note
    let get_result = get_note(&api_client, "nonexist", "json").await;
    assert!(
        get_result.is_err(),
        "Should return error for non-existent note"
    );
    let error = get_result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );

    // UPDATE: Non-existent note
    let update_request = UpdateNoteRequest {
        title: Some("New Title".to_string()),
        content: Some("New content".to_string()),
        tags: Some(vec!["test".to_string()]),
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    let update_result = update_note(&api_client, "nonexist", update_request).await;
    assert!(
        update_result.is_err(),
        "Should return error for non-existent note"
    );
    let error = update_result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );

    // DELETE: Non-existent note (with force)
    let delete_result = delete_note(&api_client, "nonexist", true).await;
    assert!(
        delete_result.is_err(),
        "Should return error for non-existent note"
    );
    let error = delete_result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}
