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
    let state = AppState::new(db);
    routes::create_router(state)
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
                .uri("/v1/task-lists")
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
                    .uri("/v1/task-lists")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": format!("List {}", i)})).unwrap(),
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
                .uri("/v1/task-lists?limit=2&offset=0")
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
                .uri("/v1/task-lists?limit=2&offset=4")
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
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Work Tasks",
                        "tags": ["work", "urgent"]
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
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Personal Tasks",
                        "tags": ["personal"]
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
                .uri("/v1/task-lists?tags=work")
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
                    .uri("/v1/task-lists")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": name})).unwrap(),
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
                .uri("/v1/task-lists?sort=name&order=asc")
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
                .uri("/v1/task-lists?sort=name&order=desc")
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
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Sprint 1",
                        "description": "First sprint tasks",
                        "tags": ["work", "urgent"],
                        "external_ref": "JIRA-123"
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
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Quick List"})).unwrap(),
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
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Test List"})).unwrap(),
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
                .uri(format!("/v1/task-lists/{}", id))
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
                .uri("/v1/task-lists/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
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
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Original"})).unwrap(),
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
                .uri(format!("/v1/task-lists/{}", id))
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
                .uri("/v1/task-lists/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "Wont Work"})).unwrap(),
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
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "To Delete"})).unwrap(),
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
                .uri(format!("/v1/task-lists/{}", id))
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
                .uri(format!("/v1/task-lists/{}", id))
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
                .uri("/v1/task-lists/nonexist")
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
                .uri("/v1/projects")
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
                .uri("/v1/repos")
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
                .uri("/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Test TaskList",
                        "description": "For relationship testing"
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
                .uri(format!("/v1/task-lists/{}", task_list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Updated TaskList with Relationships",
                        "description": "Testing relationship updates",
                        "repo_ids": [repo_id],
                        "project_ids": [project_id]
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
    assert!(
        body["project_ids"].is_array(),
        "project_ids should be included in response"
    );

    assert_eq!(body["repo_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["project_ids"].as_array().unwrap().len(), 1);
    assert_eq!(body["repo_ids"][0], repo_id);
    assert_eq!(body["project_ids"][0], project_id);
}
