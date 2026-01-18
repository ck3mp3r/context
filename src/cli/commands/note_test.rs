use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::cli::commands::PageParams;
use crate::cli::commands::note::*;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use serde_json::json;
use tokio::net::TcpListener;

// =============================================================================
// Integration Tests - Test CLI commands against real HTTP server
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
async fn test_create_note_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create note with content, tags, and project link
    let request = CreateNoteRequest {
        title: "Integration Test Note".to_string(),
        content: "This is test content for integration testing".to_string(),
        tags: Some(vec![
            "rust".to_string(),
            "testing".to_string(),
            "integration".to_string(),
        ]),
        parent_id: None,
        idx: None,
        project_ids: Some(vec![project_id.clone()]),
        repo_ids: None,
    };
    let result = create_note(&api_client, request).await;

    assert!(result.is_ok());
    let output = result.unwrap();

    // Extract note ID from success message: "✓ Created note: Title (note_id)"
    let note_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract note ID");

    // Verify all fields were persisted correctly by fetching the note
    let get_result = get_note(&api_client, note_id, "json")
        .await
        .expect("Failed to get note");
    let created_note: serde_json::Value = serde_json::from_str(&get_result).unwrap();

    assert_eq!(created_note["title"], "Integration Test Note");
    assert_eq!(
        created_note["content"],
        "This is test content for integration testing"
    );
    assert_eq!(
        created_note["tags"],
        json!(["rust", "testing", "integration"])
    );
    assert_eq!(created_note["project_ids"], json!([project_id]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_integration() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create two notes
    let req1 = CreateNoteRequest {
        title: "Note 1".to_string(),
        content: "Content 1".to_string(),
        tags: Some(vec!["tag1".to_string()]),
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    create_note(&api_client, req1)
        .await
        .expect("Failed to create note 1");

    let req2 = CreateNoteRequest {
        title: "Note 2".to_string(),
        content: "Content 2".to_string(),
        tags: Some(vec!["tag2".to_string()]),
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    create_note(&api_client, req2)
        .await
        .expect("Failed to create note 2");

    // List notes
    let result = list_notes(
        &api_client,
        None,
        None,
        None,
        None,
        None,
        PageParams::default(),
        "json",
    )
    .await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("Failed to parse JSON");

    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["title"], "Note 1");
    assert_eq!(parsed[1]["title"], "Note 2");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_note_integration() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create note
    let request = CreateNoteRequest {
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    let create_result = create_note(&api_client, request)
        .await
        .expect("Failed to create note");

    // Extract note ID from success message
    let note_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract note ID");

    // Get note
    let result = get_note(&api_client, note_id, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let note: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(note["title"], "Test Note");
    assert_eq!(note["content"], "Test content");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_note_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create note without project link
    let request = CreateNoteRequest {
        title: "Original Title".to_string(),
        content: "Original content".to_string(),
        tags: Some(vec!["tag1".to_string()]),
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    let create_result = create_note(&api_client, request)
        .await
        .expect("Failed to create note");

    let note_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract note ID");

    // Update note to add project link
    let update_request = UpdateNoteRequest {
        title: Some("Updated Title".to_string()),
        content: Some("Updated content".to_string()),
        tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
        parent_id: None,
        idx: None,
        project_ids: Some(vec![project_id.clone()]),
        repo_ids: None,
    };
    let result = update_note(&api_client, note_id, update_request).await;
    assert!(result.is_ok());

    // Verify updates including project link
    let get_result = get_note(&api_client, note_id, "json")
        .await
        .expect("Failed to get note");
    let updated_note: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_note["title"], "Updated Title");
    assert_eq!(updated_note["content"], "Updated content");
    assert_eq!(updated_note["tags"], json!(["tag1", "tag2"]));
    assert_eq!(updated_note["project_ids"], json!([project_id]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_note_integration() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create note
    let request = CreateNoteRequest {
        title: "Note to Delete".to_string(),
        content: "Content".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    let create_result = create_note(&api_client, request)
        .await
        .expect("Failed to create note");

    let note_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract note ID");

    // Delete without force should fail
    let result = delete_note(&api_client, note_id, false).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("--force"));

    // Delete with force should succeed
    let result = delete_note(&api_client, note_id, true).await;
    assert!(result.is_ok());

    // Verify note is deleted
    let get_result = get_note(&api_client, note_id, "json").await;
    assert!(get_result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_hierarchical_notes_integration() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create parent note
    let parent_request = CreateNoteRequest {
        title: "Parent Note".to_string(),
        content: "Parent content".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    let parent_result = create_note(&api_client, parent_request)
        .await
        .expect("Failed to create parent note");

    let parent_id = parent_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract parent ID");

    // Create multiple child notes with different idx values to test ordering
    let child1_request = CreateNoteRequest {
        title: "Child Note 1".to_string(),
        content: "First child".to_string(),
        tags: None,
        parent_id: Some(parent_id.to_string()),
        idx: Some(2),
        project_ids: None,
        repo_ids: None,
    };
    let child1_result = create_note(&api_client, child1_request).await;
    assert!(child1_result.is_ok());

    let child2_request = CreateNoteRequest {
        title: "Child Note 2".to_string(),
        content: "Second child".to_string(),
        tags: None,
        parent_id: Some(parent_id.to_string()),
        idx: Some(1),
        project_ids: None,
        repo_ids: None,
    };
    let child2_result = create_note(&api_client, child2_request).await;
    assert!(child2_result.is_ok());

    let child2_id = child2_result
        .unwrap()
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract child2 ID")
        .to_string();

    // Verify child note has parent_id and idx
    let get_result = get_note(&api_client, &child2_id, "json")
        .await
        .expect("Failed to get child note");
    let child_note: serde_json::Value = serde_json::from_str(&get_result).unwrap();
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

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);
    // Verify ordering by idx: Child Note 2 (idx=1) should come before Child Note 1 (idx=2)
    assert_eq!(parsed[0]["title"], "Child Note 2");
    assert_eq!(parsed[0]["idx"], 1);
    assert_eq!(parsed[1]["title"], "Child Note 1");
    assert_eq!(parsed[1]["idx"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_filters_integration() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create notes with different tags
    let rust_request = CreateNoteRequest {
        title: "Rust Note".to_string(),
        content: "Content".to_string(),
        tags: Some(vec!["rust".to_string(), "programming".to_string()]),
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    create_note(&api_client, rust_request)
        .await
        .expect("Failed to create note 1");

    let testing_request = CreateNoteRequest {
        title: "Testing Note".to_string(),
        content: "Content".to_string(),
        tags: Some(vec!["testing".to_string(), "qa".to_string()]),
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    create_note(&api_client, testing_request)
        .await
        .expect("Failed to create note 2");

    // Filter by tags
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
    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    // Find the rust note in results
    let rust_note = parsed
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["title"] == "Rust Note");
    assert!(rust_note.is_some(), "Should find Rust note in results");
}

// =============================================================================
// Project and Repository Linking Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_create_note_with_multiple_project_ids() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create a second project
    let project2_payload = serde_json::json!({"title": "Second Project"});
    let project2_response = api_client
        .post("/api/v1/projects")
        .json(&project2_payload)
        .send()
        .await
        .expect("Failed to create project 2");
    let project2: serde_json::Value = project2_response.json().await.unwrap();
    let project2_id = project2["id"].as_str().unwrap();

    // Create note linked to multiple projects
    let request = CreateNoteRequest {
        title: "Note with Multiple Projects".to_string(),
        content: "Linked to multiple projects".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        project_ids: Some(vec![project_id.clone(), project2_id.to_string()]),
        repo_ids: None,
    };
    let create_result = create_note(&api_client, request).await;
    assert!(
        create_result.is_ok(),
        "Should create note with multiple project_ids"
    );

    let output = create_result.unwrap();
    let note_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract note ID");

    // Verify both projects are linked
    let get_result = get_note(&api_client, note_id, "json").await;
    assert!(get_result.is_ok());
    let note: serde_json::Value = serde_json::from_str(&get_result.unwrap()).unwrap();
    let project_ids_val = note["project_ids"].as_array().unwrap();
    assert_eq!(project_ids_val.len(), 2);
    assert!(project_ids_val.contains(&json!(project_id)));
    assert!(project_ids_val.contains(&json!(project2_id)));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_note_with_repo_ids() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create a repository
    let repo_payload = serde_json::json!({
        "remote": "https://github.com/test/repo-for-notes",
        "tags": []
    });
    let repo_response = api_client
        .post("/api/v1/repos")
        .json(&repo_payload)
        .send()
        .await
        .expect("Failed to create repo");
    let repo: serde_json::Value = repo_response.json().await.unwrap();
    let repo_id = repo["id"].as_str().unwrap();

    // Create note with repo_ids
    let request = CreateNoteRequest {
        title: "Note with Repo Link".to_string(),
        content: "This note is linked to a repository".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: Some(vec![repo_id.to_string()]),
    };
    let create_result = create_note(&api_client, request).await;
    assert!(create_result.is_ok(), "Should create note with repo_ids");

    let output = create_result.unwrap();
    let note_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract note ID");

    // Verify repo link persists
    let get_result = get_note(&api_client, note_id, "json").await;
    assert!(get_result.is_ok());
    let note: serde_json::Value = serde_json::from_str(&get_result.unwrap()).unwrap();
    let repo_ids_val = note["repo_ids"].as_array().unwrap();
    assert_eq!(repo_ids_val.len(), 1);
    assert_eq!(repo_ids_val[0], json!(repo_id));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_note_with_both_project_and_repo_ids() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create a repository
    let repo_payload = serde_json::json!({
        "remote": "https://github.com/test/combo-repo",
        "tags": []
    });
    let repo_response = api_client
        .post("/api/v1/repos")
        .json(&repo_payload)
        .send()
        .await
        .expect("Failed to create repo");
    let repo: serde_json::Value = repo_response.json().await.unwrap();
    let repo_id = repo["id"].as_str().unwrap();

    // Create note with both project_ids and repo_ids
    let request = CreateNoteRequest {
        title: "Note with Project and Repo".to_string(),
        content: "Linked to both project and repository".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        project_ids: Some(vec![project_id.clone()]),
        repo_ids: Some(vec![repo_id.to_string()]),
    };
    let create_result = create_note(&api_client, request).await;
    assert!(
        create_result.is_ok(),
        "Should create note with both project_ids and repo_ids"
    );

    let output = create_result.unwrap();
    let note_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract note ID");

    // Verify both are linked
    let get_result = get_note(&api_client, note_id, "json").await;
    assert!(get_result.is_ok());
    let note: serde_json::Value = serde_json::from_str(&get_result.unwrap()).unwrap();

    let project_ids_val = note["project_ids"].as_array().unwrap();
    assert_eq!(project_ids_val.len(), 1);
    assert_eq!(project_ids_val[0], json!(project_id));

    let repo_ids_val = note["repo_ids"].as_array().unwrap();
    assert_eq!(repo_ids_val.len(), 1);
    assert_eq!(repo_ids_val[0], json!(repo_id));
}

// =============================================================================
// Unhappy Path Tests - NOT FOUND Errors
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_get_note_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to get non-existent note
    let result = get_note(&api_client, "nonexist", "json").await;

    // Should return error with not found message
    assert!(result.is_err(), "Should return error for non-existent note");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_note_not_found() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to update non-existent note
    let update_request = UpdateNoteRequest {
        title: Some("New Title".to_string()),
        content: Some("New content".to_string()),
        tags: None,
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    let result = update_note(&api_client, "nonexist", update_request).await;

    // Should return error
    assert!(result.is_err(), "Should return error for non-existent note");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_note_not_found_with_force() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Try to delete non-existent note with --force
    let result = delete_note(&api_client, "nonexist", true).await;

    // Should return error
    assert!(result.is_err(), "Should return error for non-existent note");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("not found") || error.contains("404") || error.contains("Not Found"),
        "Error should mention not found, got: {}",
        error
    );
}

// =============================================================================
// Unhappy Path Tests - Validation Errors
// =============================================================================

// NOTE: The following validation tests are NOT included because the API does not validate these cases:
// - test_create_note_empty_title: API allows empty titles (no validation at HTTP API layer)
// - test_create_note_with_nonexistent_parent_id: API allows nonexistent parent_id (no FK validation)
// - test_create_note_exceeds_max_content_size: No hard limit enforced at API layer

// =============================================================================
// Unhappy Path Tests - Edge Cases
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_nonexistent_tag() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create note with tags
    let request = CreateNoteRequest {
        title: "Integration Test Note".to_string(),
        content: "This is test content for integration testing".to_string(),
        tags: Some(vec![
            "rust".to_string(),
            "testing".to_string(),
            "integration".to_string(),
        ]),
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    create_note(&api_client, request)
        .await
        .expect("Failed to create note");

    // Filter by non-existent tag - should not error
    let result = list_notes(
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

    // Should succeed (doesn't error) - API returns results
    assert!(
        result.is_ok(),
        "Filtering by nonexistent tag should not error"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_offset() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create 3 notes
    for i in 1..=3 {
        let request = CreateNoteRequest {
            title: format!("Note {}", i),
            content: format!("Content {}", i),
            tags: None,
            parent_id: None,
            idx: None,
            project_ids: None,
            repo_ids: None,
        };
        let _ = create_note(&api_client, request).await;
    }

    // List with offset=1 (skip first note)
    let page = PageParams {
        limit: None,
        offset: Some(1),
        sort: None,
        order: None,
    };
    let result = list_notes(&api_client, None, None, None, None, None, page, "json").await;
    assert!(result.is_ok(), "List with offset should succeed");

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        parsed.as_array().unwrap().len(),
        2,
        "Should return 2 notes after skipping 1"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_sort_and_order() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create notes with different titles
    let req1 = CreateNoteRequest {
        title: "Zebra Note".to_string(),
        content: "Content".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    let _ = create_note(&api_client, req1).await;

    let req2 = CreateNoteRequest {
        title: "Alpha Note".to_string(),
        content: "Content".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    let _ = create_note(&api_client, req2).await;

    let req3 = CreateNoteRequest {
        title: "Beta Note".to_string(),
        content: "Content".to_string(),
        tags: None,
        parent_id: None,
        idx: None,
        project_ids: None,
        repo_ids: None,
    };
    let _ = create_note(&api_client, req3).await;

    // List sorted by title ascending
    let page = PageParams {
        limit: None,
        offset: None,
        sort: Some("title"),
        order: Some("asc"),
    };
    let result = list_notes(&api_client, None, None, None, None, None, page, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let notes = parsed.as_array().unwrap();

    assert_eq!(notes.len(), 3);
    assert_eq!(notes[0]["title"], "Alpha Note");
    assert_eq!(notes[1]["title"], "Beta Note");
    assert_eq!(notes[2]["title"], "Zebra Note");

    // List sorted by title descending
    let page = PageParams {
        limit: None,
        offset: None,
        sort: Some("title"),
        order: Some("desc"),
    };
    let result = list_notes(&api_client, None, None, None, None, None, page, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let notes = parsed.as_array().unwrap();

    assert_eq!(notes.len(), 3);
    assert_eq!(notes[0]["title"], "Zebra Note");
    assert_eq!(notes[1]["title"], "Beta Note");
    assert_eq!(notes[2]["title"], "Alpha Note");
}
