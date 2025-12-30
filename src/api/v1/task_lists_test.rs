//! Integration tests for TaskList API endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::{AppState, routes};
use crate::db::{Database, SqliteDatabase};

async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

    // Create a test project with known ID for API tests
    sqlx::query("INSERT OR IGNORE INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("test0000")
        .bind("Test Project")
        .bind("Default project for tests")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Create test project should succeed");

    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
    );
    routes::create_router(state, false)
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// GET /v1/task-lists - List TaskLists (with tags, pagination, ordering)
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_task_lists_initially_empty() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert!(body["items"].as_array().unwrap().is_empty());
    assert_eq!(body["total"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_task_lists_with_pagination() {
    let app = test_app().await;

    // Create 5 task lists
    for i in 1..=5 {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/task-lists")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(
                            &json!({"name": format!("List {}", i), "project_id": "test0000"}),
                        )
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Get first page
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?limit=2&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    assert_eq!(body["total"], 5);
    assert_eq!(body["limit"], 2);
    assert_eq!(body["offset"], 0);

    // Get last page
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?limit=2&offset=4")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_task_lists_with_tag_filter() {
    let app = test_app().await;

    // Create task lists with different tags
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Work Tasks",
                        "tags": ["work", "urgent"],
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Personal Tasks",
                        "tags": ["personal"],
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter by "work" tag
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?tags=work")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["name"], "Work Tasks");
}

#[tokio::test(flavor = "multi_thread")]
async fn list_task_lists_with_ordering() {
    let app = test_app().await;

    // Create task lists
    for name in ["Zebra", "Apple", "Mango"] {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/task-lists")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": name, "project_id": "test0000"}))
                            .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Sort by name ascending
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?sort=name&order=asc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items[0]["name"], "Apple");
    assert_eq!(items[1]["name"], "Mango");
    assert_eq!(items[2]["name"], "Zebra");

    // Sort by name descending
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?sort=name&order=desc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items[0]["name"], "Zebra");
}

// =============================================================================
// POST /v1/task-lists - Create TaskList
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_task_list_returns_created() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Sprint 1",
                        "description": "First sprint tasks",
                        "tags": ["work", "urgent"],
                        "external_ref": "JIRA-123",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["name"], "Sprint 1");
    assert_eq!(body["description"], "First sprint tasks");
    assert_eq!(body["tags"], json!(["work", "urgent"]));
    assert_eq!(body["external_ref"], "JIRA-123");
    assert_eq!(body["status"], "active");
    assert!(body["id"].as_str().unwrap().len() == 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn create_task_list_minimal() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Quick List", "project_id": "test0000"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["name"], "Quick List");
    assert!(body["description"].is_null());
    assert_eq!(body["tags"], json!([]));
}

// =============================================================================
// GET /v1/task-lists/{id} - Get TaskList
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_task_list_returns_task_list() {
    let app = test_app().await;

    // Create first
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Test List", "project_id": "test0000"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let id = body["id"].as_str().unwrap();

    // Get it
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["id"], id);
    assert_eq!(body["name"], "Test List");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_task_list_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_task_list_loads_relationships() {
    let app = test_app().await;

    // Create a repo to link to
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:test/repo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let repo_body = json_body(response).await;
    let repo_id = repo_body["id"].as_str().unwrap();

    // Create a task list with relationships
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Test List with Relationships",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(response).await;
    let list_id = created["id"].as_str().unwrap();

    // Update to add repo relationship
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/task-lists/{}", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Test List with Relationships",
                        "repo_ids": [repo_id],
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // GET the task list and verify relationships are loaded
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;

    // Verify project_id is loaded
    assert_eq!(body["project_id"], "test0000");

    // Verify repo_ids is loaded
    assert!(body["repo_ids"].is_array(), "repo_ids should be an array");
    assert_eq!(body["repo_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["repo_ids"][0], repo_id);
}

// =============================================================================
// PUT /v1/task-lists/{id} - Update TaskList
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn update_task_list_returns_updated() {
    let app = test_app().await;

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Original", "project_id": "test0000"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let id = body["id"].as_str().unwrap();

    // Update
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/task-lists/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Updated",
                        "description": "Now with description",
                        "status": "archived"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["name"], "Updated");
    assert_eq!(body["description"], "Now with description");
    assert_eq!(body["status"], "archived");
}

#[tokio::test(flavor = "multi_thread")]
async fn update_task_list_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/task-lists/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Wont Work", "project_id": "test0000"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// DELETE /v1/task-lists/{id} - Delete TaskList
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn delete_task_list_returns_no_content() {
    let app = test_app().await;

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "To Delete", "project_id": "test0000"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let id = body["id"].as_str().unwrap();

    // Delete
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/task-lists/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify gone
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_task_list_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/task-lists/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_task_list_handles_relationships() {
    let app = test_app().await;

    // First create a project and repo to link to
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Test Project",
                        "description": "For relationship testing"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let project_body = json_body(project_response).await;
    let project_id = project_body["id"].as_str().unwrap();

    let repo_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "remote": "github:user/test-repo",
                        "path": "/test/path"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let repo_body = json_body(repo_response).await;
    let repo_id = repo_body["id"].as_str().unwrap();

    // Create task list without relationships
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Personal Tasks",
                        "tags": ["personal"],
                        "project_id": "test0000"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let create_body = json_body(create_response).await;
    let task_list_id = create_body["id"].as_str().unwrap();

    // Update task list WITH relationships - this should work but currently doesn't
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/task-lists/{}", task_list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Updated TaskList with Relationships",
                        "description": "Testing relationship updates",
                        "repo_ids": [repo_id],
                        "project_id": project_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;

    // Verify the task list was updated
    assert_eq!(body["name"], "Updated TaskList with Relationships");
    assert_eq!(body["description"], "Testing relationship updates");

    // Verify relationships are included and correct
    assert!(
        body["repo_ids"].is_array(),
        "repo_ids should be included in response"
    );
    assert_eq!(body["project_id"], project_id);

    assert_eq!(body["repo_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["repo_ids"][0], repo_id);
}

// =============================================================================
// PATCH /v1/task-lists/{id} - Partial Update TaskList
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn patch_task_list_partial_name_update() {
    let app = test_app().await;

    // Create a task list
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Original Name",
                        "description": "Original Description",
                        "tags": ["original", "tag"],
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let id = body["id"].as_str().unwrap();

    // Partially update only the name
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/task-lists/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Updated Name"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;

    // Name should be updated
    assert_eq!(body["name"], "Updated Name");

    // Other fields should remain unchanged
    assert_eq!(body["description"], "Original Description");
    assert_eq!(body["tags"], json!(["original", "tag"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_task_list_status_to_archived_sets_archived_at() {
    let app = test_app().await;

    // Create an active task list
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Active List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let id = body["id"].as_str().unwrap();

    // Verify initially active with no archived_at
    assert_eq!(body["status"], "active");
    assert!(body["archived_at"].is_null());

    // PATCH status to archived
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/task-lists/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "status": "archived"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;

    // Status should be archived
    assert_eq!(body["status"], "archived");

    // archived_at should be set automatically by repository layer
    assert!(!body["archived_at"].is_null());
    assert!(!body["archived_at"].as_str().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_task_list_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/task-lists/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Won't Work"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// GET /v1/task-lists?project_id=X - Filter by Project
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn list_task_lists_filtered_by_project_id() {
    let app = test_app().await;

    // Create two projects
    let project_a_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project A",
                        "description": "First project"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let project_a_body = json_body(project_a_response).await;
    let project_a_id = project_a_body["id"].as_str().unwrap();

    let project_b_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Project B",
                        "description": "Second project"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let project_b_body = json_body(project_b_response).await;
    let project_b_id = project_b_body["id"].as_str().unwrap();

    // Create task lists: 2 for project A, 1 for project B
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Project A - Sprint 1",
                        "project_id": project_a_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Project A - Sprint 2",
                        "project_id": project_a_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Project B - Sprint 1",
                        "project_id": project_b_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter by project_id for Project A
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists?project_id={}", project_a_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);

    // Verify all returned items belong to Project A
    for item in body["items"].as_array().unwrap() {
        assert_eq!(item["project_id"], project_a_id);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn list_task_lists_filtered_by_nonexistent_project() {
    let app = test_app().await;

    // Create a task list with the default project
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Some Task List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter by a non-existent project_id
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/task-lists?project_id=nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn list_task_lists_filtered_by_project_id_and_tags() {
    let app = test_app().await;

    // Create a project
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test Project"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let project_body = json_body(project_response).await;
    let project_id = project_body["id"].as_str().unwrap();

    // Create task lists with various tag combinations
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Work List",
                        "tags": ["work", "urgent"],
                        "project_id": project_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Personal List",
                        "tags": ["personal"],
                        "project_id": project_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Different Project Work",
                        "tags": ["work"],
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter by project_id AND tags=work
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/task-lists?project_id={}&tags=work",
                    project_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["name"], "Work List");
    assert_eq!(body["items"][0]["project_id"], project_id);
}

// =============================================================================
// PATCH /v1/task-lists/{id} - Relationship Relinking
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn patch_task_list_link_to_project_and_repo() {
    let app = test_app().await;

    // Create a project
    let project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Test Project"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let project_body = json_body(project_response).await;
    let project_id = project_body["id"].as_str().unwrap().to_string();

    // Create a repo
    let repo_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "remote": "github:user/test-repo"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let repo_body = json_body(repo_response).await;
    let repo_id = repo_body["id"].as_str().unwrap().to_string();

    // Create a task list without relationships
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Test List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let list_body = json_body(list_response).await;
    let list_id = list_body["id"].as_str().unwrap();

    // Verify initial relationships
    assert_eq!(list_body["project_id"], "test0000");
    assert!(list_body["repo_ids"].as_array().unwrap().is_empty());

    // PATCH to link to both project and repo
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/task-lists/{}", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "project_id": project_id,
                        "repo_ids": [repo_id]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;

    // Verify relationships were added
    assert_eq!(body["project_id"], project_id);
    assert_eq!(body["repo_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["repo_ids"][0], repo_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_task_list_stats_returns_counts_by_status() {
    let app = test_app().await;

    // Create task list
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Stats Test List",
                        "project_id": "test0000"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let list_body = json_body(create_response).await;
    let list_id = list_body["id"].as_str().unwrap();

    // Create tasks with different statuses
    for (i, status) in [
        "backlog",
        "todo",
        "todo",
        "in_progress",
        "done",
        "done",
        "done",
    ]
    .iter()
    .enumerate()
    {
        let create_task_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "content": format!("Task {}", i),
                            "status": status
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            create_task_response.status(),
            StatusCode::CREATED,
            "Failed to create task {}",
            i
        );
    }

    // Get stats
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/task-lists/{}/stats", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;

    assert_eq!(body["list_id"], list_id);
    assert_eq!(body["total"], 7);
    assert_eq!(body["backlog"], 1);
    assert_eq!(body["todo"], 2);
    assert_eq!(body["in_progress"], 1);
    assert_eq!(body["review"], 0);
    assert_eq!(body["done"], 3);
    assert_eq!(body["cancelled"], 0);
}
