//! Integration tests for the Skill API endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::notifier::ChangeNotifier;
use crate::api::{AppState, routes};
use crate::db::{Database, SqliteDatabase};

/// Setup test app with real in-memory DB and skills API routes
async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        ChangeNotifier::new(),
    );
    routes::create_router(state, false)
}

/// Helper for parsing JSON response body
async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_empty() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
    assert_eq!(body["total"], 0);
    assert_eq!(body["limit"], 50);
    assert_eq!(body["offset"], 0);
}

// --- ==== CRUD & Validation Tests ==== ---

#[tokio::test(flavor = "multi_thread")]
async fn test_create_skill() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "test-skill",
                        "description": "A skill description",
                        "content": "---\nname: test-skill\ndescription: A skill description\n---\n\nFollow these steps",
                        "tags": ["tag1", "tag2"],
                        "project_ids": []
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_body(response).await;
    assert_eq!(created["name"], "test-skill");
    assert_eq!(created["description"], "A skill description");
    assert!(
        created["content"]
            .as_str()
            .unwrap()
            .contains("Follow these steps")
    );
    assert_eq!(created["tags"], json!(["tag1", "tag2"]));
    assert!(!created["id"].as_str().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_skill_missing_field() {
    let app = test_app().await;

    // Missing required "name"
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "description": "oops",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_by_id() {
    let app = test_app().await;
    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "skill1",
                        "description": "Test description",
                        "content": "---\nname: skill1\ndescription: Test description\n---\n\nTest instructions"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // GET
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/skills/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let got = json_body(response).await;
    assert_eq!(got["id"], id);
    assert_eq!(got["name"], "skill1");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_not_found() {
    let app = test_app().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills/skl_NONEXISTENT")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_patch() {
    let app = test_app().await;
    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "patch-me",
                        "description": "Test description",
                        "content": "---\nname: patch-me\ndescription: Test description\n---\n\nTest instructions"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // PATCH
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/skills/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "patched-name", "tags": ["edited"]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let patched = json_body(response).await;
    assert_eq!(patched["id"], id);
    assert_eq!(patched["name"], "patched-name");
    assert_eq!(patched["tags"], json!(["edited"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_not_found() {
    let app = test_app().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/skills/skl_NOTREAL")
                .header("content-type", "application/json")
                .body(Body::from(json!({"name": "x"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_put() {
    let app = test_app().await;
    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "put-me",
                        "description": "Test description",
                        "content": "---\nname: put-me\ndescription: Test description\n---\n\nTest instructions"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // PUT (full replacement)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/skills/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "put-name",
                        "description": "desc",
                        "content": "---\nname: put-name\ndescription: desc\n---\n\ninstructions"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let put = json_body(response).await;
    assert_eq!(put["id"], id);
    assert_eq!(put["name"], "put-name");
    assert_eq!(put["description"], "desc");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_skill() {
    let app = test_app().await;
    // Create
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "delete-me",
                        "description": "Test description",
                        "content": "---\nname: delete-me\ndescription: Test description\n---\n\nTest instructions"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // DELETE
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/skills/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Confirm deleted
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/skills/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_skill_not_found() {
    let app = test_app().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/skills/skl_FAKEDELETE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_non_empty() {
    let app = test_app().await;

    // Insert multiple
    for i in 0..3 {
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/skills")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": format!("Skill{}", i),
                            "description": "Test description",
                            "content": format!("---\nname: Skill{}\ndescription: Test description\n---\n\nTest instructions", i)
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 3);
    assert_eq!(body["total"], 3);
    assert_eq!(body["limit"], 50);
    assert_eq!(body["offset"], 0);
}

// Edge: Invalid UUID (should be consistent, but uses string keys like "skl_", so skip if UUID not enforced)
// Edge: Add/modify more detailed validation tests if Skill fields are restricted further

// If filtering, sorting, or pagination implemented for Skill, add tests mirroring notes_test.rs here as well.

// --- ==== Agent Skills Specification Tests ==== ---

#[tokio::test(flavor = "multi_thread")]
async fn test_create_skill_with_agent_skills_fields() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "agent-skill",
                        "description": "A skill with Agent Skills metadata",
                        "content": r#"---
name: agent-skill
description: A skill with Agent Skills metadata
license: MIT
compatibility: opencode>=0.1.0
allowed-tools: ["read", "write", "edit"]
metadata:
  author: test
  version: "1.0.0"
origin:
  url: https://github.com/example/skill
  ref: main
---

Follow these steps
"#,
                        "tags": ["agent", "spec"],
                        "project_ids": []
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_body(response).await;
    assert_eq!(created["name"], "agent-skill");
    // Verify Agent Skills fields are in content
    let content = created["content"].as_str().unwrap();
    assert!(content.contains("license: MIT"));
    assert!(content.contains("compatibility: opencode>=0.1.0"));
    assert!(content.contains(r#"["read", "write", "edit"]"#));
    assert!(content.contains("author: test"));
    assert!(content.contains(r#"version: "1.0.0""#));
    assert!(content.contains("url: https://github.com/example/skill"));
    assert!(content.contains("ref: main"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_returns_agent_skills_fields() {
    let app = test_app().await;

    // Create skill with all fields
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "full-skill",
                        "description": "Complete skill",
                        "content": r#"---
name: full-skill
description: Complete skill
license: Apache-2.0
compatibility: opencode>=0.2.0
allowed-tools: ["grep", "glob"]
metadata:
  category: development
origin:
  url: git://example.com/repo.git
  ref: v1.0.0
---

Instructions
"#
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // GET should return all fields
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/skills/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let got = json_body(response).await;
    assert_eq!(got["id"], id);
    assert_eq!(got["name"], "full-skill");
    // Verify Agent Skills fields are in content
    let content = got["content"].as_str().unwrap();
    assert!(content.contains("license: Apache-2.0"));
    assert!(content.contains("compatibility: opencode>=0.2.0"));
    assert!(content.contains(r#"["grep", "glob"]"#));
    assert!(content.contains("category: development"));
    assert!(content.contains("url: git://example.com/repo.git"));
    assert!(content.contains("ref: v1.0.0"));
    // origin_fetched_at and origin_metadata are auto-managed, not user-provided
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_skill_agent_skills_fields() {
    let app = test_app().await;

    // Create minimal skill
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "patch-test",
                        "description": "Initial description",
                        "content": "---\nname: patch-test\ndescription: Initial description\n---\n\nInitial instructions"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // PATCH with Agent Skills fields
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/skills/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "content": r#"---
name: patch-test
description: Initial description
license: MIT
compatibility: opencode>=1.0.0
allowed-tools: ["task"]
origin:
  url: https://agentskills.io/skills/example
---

Initial instructions
"#
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let patched = json_body(response).await;
    assert_eq!(patched["id"], id);
    // Verify Agent Skills fields are in updated content
    let content = patched["content"].as_str().unwrap();
    assert!(content.contains("license: MIT"));
    assert!(content.contains("compatibility: opencode>=1.0.0"));
    assert!(content.contains(r#"["task"]"#));
    assert!(content.contains("url: https://agentskills.io/skills/example"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_skill_agent_skills_fields() {
    let app = test_app().await;

    // Create skill
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "put-test",
                        "description": "Test",
                        "content": "---\nname: put-test\ndescription: Test\n---\n\nTest"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = json_body(response).await;
    let id = created["id"].as_str().unwrap();

    // PUT (full replacement) with Agent Skills fields
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/skills/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "replaced-skill",
                        "description": "Replaced",
                        "content": r#"---
name: replaced-skill
description: Replaced
license: GPL-3.0
metadata:
  type: utility
---

New instructions
"#
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let put = json_body(response).await;
    assert_eq!(put["id"], id);
    assert_eq!(put["name"], "replaced-skill");
    // Verify Agent Skills fields are in content
    let content = put["content"].as_str().unwrap();
    assert!(content.contains("license: GPL-3.0"));
    assert!(content.contains("type: utility"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_with_query_performs_search() {
    let app = test_app().await;

    // Create two skills
    let rust_skill = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "rust-programming",
                        "description": "Systems programming with Rust",
                        "content": r#"---
name: rust-programming
description: Systems programming with Rust
---

# Rust Programming

Learn systems programming with Rust.
"#,
                        "tags": ["programming", "systems"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rust_skill.status(), StatusCode::CREATED);

    let python_skill = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "python-web",
                        "description": "Web development with Python",
                        "content": r#"---
name: python-web
description: Web development with Python
---

# Python Web Development

Learn web programming with Python and Django.
"#,
                        "tags": ["programming", "web"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(python_skill.status(), StatusCode::CREATED);

    // Search for "systems" - should only match rust skill
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills?q=systems")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items[0]["name"], "rust-programming");

    // Search for "web" - should only match python skill
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills?q=web")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 1);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items[0]["name"], "python-web");

    // Search for "programming" - should match both
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills?q=programming")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 2);
}
