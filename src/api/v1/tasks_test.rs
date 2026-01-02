//! Integration tests for Task API endpoints.

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
        crate::api::notifier::ChangeNotifier::new(),
    );
    routes::create_router(state, false)
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

/// Helper to create a task list and return its ID
async fn create_task_list(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Test List", "project_id": "test0000"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(response).await;
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn list_tasks_for_list() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Initially empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert!(body["items"].as_array().unwrap().is_empty());
    assert_eq!(body["total"], 0);

    // Create a task
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Do something"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Now has one task
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn create_task_returns_created() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Complete the feature",
                        "priority": 2
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["title"], "Complete the feature");
    assert_eq!(body["priority"], 2);
    assert_eq!(body["status"], "backlog");
    assert_eq!(body["list_id"], list_id);
    assert!(body["id"].as_str().unwrap().len() == 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn create_task_minimal() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Quick task"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = json_body(response).await;
    assert_eq!(body["title"], "Quick task");
    assert_eq!(body["status"], "backlog");
    assert!(body["priority"].is_null());
}

#[tokio::test(flavor = "multi_thread")]
async fn get_task_returns_task() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Test task"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let task_id = body["id"].as_str().unwrap();

    // Get
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["id"], task_id);
    assert_eq!(body["title"], "Test task");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_task_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/tasks/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_task_move_to_different_list() {
    let app = test_app().await;

    // Create two task lists
    let list1_id = create_task_list(&app).await;
    let list2_id = create_task_list(&app).await;

    // Create task in list1
    let task_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list1_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Task to move"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(task_response.status(), StatusCode::CREATED);
    let task_body = json_body(task_response).await;
    let task_id = task_body["id"].as_str().unwrap();
    assert_eq!(task_body["list_id"], list1_id);

    // Move task to list2 using PATCH
    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "list_id": list2_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);
    let patched_body = json_body(patch_response).await;

    // Verify task moved to list2
    assert_eq!(patched_body["list_id"], list2_id);
    assert_eq!(patched_body["title"], "Task to move"); // Content unchanged
    assert_eq!(patched_body["status"], "backlog"); // Status unchanged
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_task_status_to_done_sets_completed_at() {
    let app = test_app().await;

    // Create task list and task
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({"title": "Test List", "project_id": "test0000"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let list = json_body(list_response).await;
    let list_id = list["id"].as_str().unwrap();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Task to complete"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(create_response).await;
    let task_id = created["id"].as_str().unwrap();
    assert!(created["completed_at"].is_null());

    // PATCH status to done
    let patch_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "status": "done"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);
    let patched = json_body(patch_response).await;

    assert_eq!(patched["status"], "done");
    assert!(
        patched["completed_at"].is_string(),
        "completed_at should be auto-set when status changes to done"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn patch_task_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/tasks/notfound")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({"title": "New"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_task_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/tasks/nonexist")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "Wont work"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_task_with_tags() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Create a task without tags
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Task without tags"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_body(response).await;
    let task_id = created["id"].as_str().unwrap();

    // Verify task has no tags initially
    assert_eq!(created["tags"], json!([]));

    // Update task with tags using PUT
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated task with tags",
                        "tags": ["updated", "bug-fix", "urgent"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let updated = json_body(response).await;

    // Verify content was updated
    assert_eq!(updated["title"], "Updated task with tags");

    // Verify tags were set correctly via PUT
    assert_eq!(updated["tags"], json!(["updated", "bug-fix", "urgent"]));
    assert_eq!(updated["tags"].as_array().unwrap().len(), 3);

    // Update again with different tags to verify replacement (not merge)
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Updated task with different tags",
                        "tags": ["production", "critical"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let updated2 = json_body(response).await;

    // Verify tags were replaced, not merged
    assert_eq!(updated2["tags"], json!(["production", "critical"]));
    assert_eq!(updated2["tags"].as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_task_returns_no_content() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"title": "To delete"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let task_id = body["id"].as_str().unwrap();

    // Delete
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/tasks/{}", task_id))
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
                .uri(format!("/api/v1/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_task_not_found() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/tasks/nonexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Cascade status update integration tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn api_cascade_status_to_matching_subtasks() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Create parent task (status: backlog)
    let parent_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Parent task",
                        "status": "backlog"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let parent = json_body(parent_response).await;
    let parent_id = parent["id"].as_str().unwrap();

    // Create 2 subtasks (status: backlog, matching parent)
    let subtask1_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Subtask 1",
                        "status": "backlog",
                        "parent_id": parent_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let subtask1 = json_body(subtask1_response).await;
    let subtask1_id = subtask1["id"].as_str().unwrap();

    let subtask2_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Subtask 2",
                        "status": "backlog",
                        "parent_id": parent_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let subtask2 = json_body(subtask2_response).await;
    let subtask2_id = subtask2["id"].as_str().unwrap();

    // Update parent: backlog → done
    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", parent_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"status": "done"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_response.status(), StatusCode::OK);

    // Verify subtasks cascaded to done
    let subtask1_get = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tasks/{}", subtask1_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let subtask1_updated = json_body(subtask1_get).await;
    assert_eq!(
        subtask1_updated["status"], "done",
        "Subtask 1 should cascade to done"
    );

    let subtask2_get = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tasks/{}", subtask2_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let subtask2_updated = json_body(subtask2_get).await;
    assert_eq!(
        subtask2_updated["status"], "done",
        "Subtask 2 should cascade to done"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn api_cascade_only_matching_subtasks() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Create parent (status: backlog)
    let parent_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Parent task",
                        "status": "backlog"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let parent = json_body(parent_response).await;
    let parent_id = parent["id"].as_str().unwrap();

    // Create matching subtask (status: backlog)
    let matching_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Matching subtask",
                        "status": "backlog",
                        "parent_id": parent_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let matching = json_body(matching_response).await;
    let matching_id = matching["id"].as_str().unwrap();

    // Create diverged subtask (status: in_progress, different from parent)
    let diverged_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "title": "Diverged subtask",
                        "parent_id": parent_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let diverged = json_body(diverged_response).await;
    let diverged_id = diverged["id"].as_str().unwrap();

    // Update diverged subtask to in_progress
    app.clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", diverged_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"status": "in_progress"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Update parent: backlog → done
    app.clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/tasks/{}", parent_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"status": "done"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Verify: Matching subtask cascaded, diverged did not
    let matching_get = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tasks/{}", matching_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let matching_updated = json_body(matching_get).await;
    assert_eq!(
        matching_updated["status"], "done",
        "Matching subtask should cascade to done"
    );

    let diverged_get = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tasks/{}", diverged_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let diverged_updated = json_body(diverged_get).await;
    assert_eq!(
        diverged_updated["status"], "in_progress",
        "Diverged subtask should remain in_progress"
    );
}

/// Helper to create a task with specified content, parent_id, and status
async fn create_task(
    app: &axum::Router,
    list_id: &str,
    title: &str,
    parent_id: Option<&str>,
    status: &str,
) -> String {
    let mut payload = json!({"title": title});
    if let Some(pid) = parent_id {
        payload["parent_id"] = json!(pid);
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = json_body(response).await;
    let task_id = body["id"].as_str().unwrap().to_string();

    // Update to desired status if not backlog
    if status != "backlog" {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/tasks/{}", task_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"status": status})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    task_id
}

#[tokio::test(flavor = "multi_thread")]
async fn test_api_type_task_returns_only_parents() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Create 2 parents (done)
    let parent1_id = create_task(&app, &list_id, "Parent 1", None, "done").await;
    create_task(&app, &list_id, "Parent 2", None, "done").await;

    // Create 2 subtasks (done)
    create_task(&app, &list_id, "Subtask 1", Some(&parent1_id), "done").await;
    create_task(&app, &list_id, "Subtask 2", Some(&parent1_id), "done").await;

    // Query with type=task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/task-lists/{}/tasks?status=done&type=task",
                    list_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2, "Should return only 2 parents");
    assert_eq!(body["items"].as_array().unwrap().len(), 2);

    // Verify all returned tasks have parent_id = null
    for item in body["items"].as_array().unwrap() {
        assert!(
            item["parent_id"].is_null(),
            "All tasks should be parents (parent_id IS NULL)"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_api_type_subtask_returns_only_subtasks() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Create 1 parent (done)
    let parent_id = create_task(&app, &list_id, "Parent", None, "done").await;

    // Create 2 subtasks (done)
    create_task(&app, &list_id, "Subtask 1", Some(&parent_id), "done").await;
    create_task(&app, &list_id, "Subtask 2", Some(&parent_id), "done").await;

    // Query with type=subtask
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/task-lists/{}/tasks?status=done&type=subtask",
                    list_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2, "Should return only 2 subtasks");
    assert_eq!(body["items"].as_array().unwrap().len(), 2);

    // Verify all returned tasks have parent_id NOT null
    for item in body["items"].as_array().unwrap() {
        assert!(
            !item["parent_id"].is_null(),
            "All tasks should be subtasks (parent_id IS NOT NULL)"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_api_type_omitted_returns_all() {
    let app = test_app().await;
    let list_id = create_task_list(&app).await;

    // Create 1 parent (done)
    let parent_id = create_task(&app, &list_id, "Parent", None, "done").await;

    // Create 1 subtask (done)
    create_task(&app, &list_id, "Subtask", Some(&parent_id), "done").await;

    // Query WITHOUT type parameter (backward compatibility)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/task-lists/{}/tasks?status=done", list_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2, "Should return both parent and subtask");
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
}

// =============================================================================
// WebSocket Broadcast Tests
// =============================================================================

async fn test_app_with_notifier() -> (axum::Router, crate::api::notifier::ChangeNotifier) {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();

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

    let notifier = crate::api::notifier::ChangeNotifier::new();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        notifier.clone(),
    );
    (routes::create_router(state, false), notifier)
}

#[tokio::test(flavor = "multi_thread")]
async fn create_task_broadcasts_notification() {
    let (app, notifier) = test_app_with_notifier().await;

    // Create a task list first
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Test List",
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

    let mut subscriber = notifier.subscribe();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "New Task"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_body(response).await;
    let task_id = created["id"].as_str().unwrap();

    // Should receive TaskCreated broadcast
    let msg = subscriber.recv().await.expect("Should receive broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::TaskCreated { task_id: id } => {
            assert_eq!(id, task_id);
        }
        _ => panic!("Expected TaskCreated message, got {:?}", msg),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn update_task_broadcasts_notification() {
    let (app, notifier) = test_app_with_notifier().await;

    // Create a task list first
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Test List",
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

    // Create a task
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Original Task"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(create_response).await;
    let task_id = created["id"].as_str().unwrap().to_string();

    // Subscribe AFTER creation to avoid receiving create notification
    let mut subscriber = notifier.subscribe();

    // Update the task
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Updated Task",
                        "description": "With description"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Should receive TaskUpdated broadcast
    let msg = subscriber.recv().await.expect("Should receive broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::TaskUpdated { task_id: id } => {
            assert_eq!(id, task_id);
        }
        _ => panic!("Expected TaskUpdated message, got {:?}", msg),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_task_broadcasts_notification() {
    let (app, notifier) = test_app_with_notifier().await;

    // Create a task list first
    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/task-lists")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Test List",
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

    // Create a task
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/task-lists/{}/tasks", list_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({
                        "title": "Task to Delete"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let created = json_body(create_response).await;
    let task_id = created["id"].as_str().unwrap().to_string();

    // Subscribe AFTER creation
    let mut subscriber = notifier.subscribe();

    // Delete the task
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Should receive TaskDeleted broadcast
    let msg = subscriber.recv().await.expect("Should receive broadcast");
    match msg {
        crate::api::notifier::UpdateMessage::TaskDeleted { task_id: id } => {
            assert_eq!(id, task_id);
        }
        _ => panic!("Expected TaskDeleted message, got {:?}", msg),
    }
}
