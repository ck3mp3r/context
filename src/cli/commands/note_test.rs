use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
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
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url));

    // Create note with content and tags
    let result = create_note(
        &api_client,
        "Integration Test Note",
        "This is test content for integration testing",
        Some("rust,testing,integration"),
        None,
        None,
    )
    .await;

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
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_integration() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create two notes
    create_note(&api_client, "Note 1", "Content 1", Some("tag1"), None, None)
        .await
        .expect("Failed to create note 1");
    create_note(&api_client, "Note 2", "Content 2", Some("tag2"), None, None)
        .await
        .expect("Failed to create note 2");

    // List notes
    let result = list_notes(&api_client, None, None, None, None, None, "json").await;
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
    let create_result = create_note(&api_client, "Test Note", "Test content", None, None, None)
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
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create note
    let create_result = create_note(
        &api_client,
        "Original Title",
        "Original content",
        Some("tag1"),
        None,
        None,
    )
    .await
    .expect("Failed to create note");

    let note_id = create_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract note ID");

    // Update note
    let result = update_note(
        &api_client,
        note_id,
        Some("Updated Title"),
        Some("Updated content"),
        Some("tag1,tag2"),
        None,
        None,
    )
    .await;
    assert!(result.is_ok());

    // Verify updates
    let get_result = get_note(&api_client, note_id, "json")
        .await
        .expect("Failed to get note");
    let updated_note: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(updated_note["title"], "Updated Title");
    assert_eq!(updated_note["content"], "Updated content");
    assert_eq!(updated_note["tags"], json!(["tag1", "tag2"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_note_integration() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create note
    let create_result = create_note(&api_client, "Note to Delete", "Content", None, None, None)
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
    let parent_result = create_note(
        &api_client,
        "Parent Note",
        "Parent content",
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create parent note");

    let parent_id = parent_result
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract parent ID");

    // Create child note with parent_id and idx
    let child_result = create_note(
        &api_client,
        "Child Note",
        "Child content",
        None,
        Some(parent_id),
        Some(1),
    )
    .await;
    assert!(child_result.is_ok());

    let child_id = child_result
        .unwrap()
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract child ID")
        .to_string();

    // Verify child note has parent_id
    let get_result = get_note(&api_client, &child_id, "json")
        .await
        .expect("Failed to get child note");
    let child_note: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    assert_eq!(child_note["parent_id"], parent_id);
    assert_eq!(child_note["idx"], 1);

    // List notes filtered by parent_id
    let result = list_notes(&api_client, None, None, Some(parent_id), None, None, "json").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["title"], "Child Note");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_notes_with_filters_integration() {
    let (url, _project_id, _handle) = spawn_test_server().await;
    let api_client = ApiClient::new(Some(url.clone()));

    // Create notes with different tags
    create_note(
        &api_client,
        "Rust Note",
        "Content",
        Some("rust,programming"),
        None,
        None,
    )
    .await
    .expect("Failed to create note 1");
    create_note(
        &api_client,
        "Testing Note",
        "Content",
        Some("testing,qa"),
        None,
        None,
    )
    .await
    .expect("Failed to create note 2");

    // Filter by tags
    let result = list_notes(&api_client, None, Some("rust"), None, None, None, "json").await;
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
    let result = update_note(
        &api_client,
        "nonexist",
        Some("New Title"),
        Some("New content"),
        None,
        None,
        None,
    )
    .await;

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
