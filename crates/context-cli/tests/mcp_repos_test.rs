//! Tests for Repository MCP tools
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use context_core::{HasRepos, Repo, RepoRepository};
use context_db::SqliteDatabase;
use context_server::api::notifier::ChangeNotifier;
use context_server::mcp::tools::repos::*;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
};
use std::sync::Arc;

async fn setup_db() -> SqliteDatabase {
    common::setup_db().await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_repos_empty() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = RepoTools::new(db, ChangeNotifier::new());
    let result = tools
        .list_repos(Parameters(ListReposParams {
            query: None,
            project_id: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_repo() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = RepoTools::new(db, ChangeNotifier::new());
    let result = tools
        .create_repo(Parameters(CreateRepoParams {
            remote: "https://github.com/test/repo".to_string(),
            path: Some("/test/path".to_string()),
            tags: Some(vec!["test".to_string()]),
            project_ids: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["remote"], "https://github.com/test/repo");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_repo_empty_remote() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = RepoTools::new(db, ChangeNotifier::new());
    let result = tools
        .create_repo(Parameters(CreateRepoParams {
            remote: "".to_string(),
            path: None,
            tags: None,
            project_ids: None,
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_repo() {
    let db = setup_db().await;
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec!["test".to_string()],
        project_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.repos().create(&repo).await.unwrap();
    let db = Arc::new(db);

    let tools = RepoTools::new(db, ChangeNotifier::new());
    let result = tools
        .get_repo(Parameters(GetRepoParams {
            id: "12345678".to_string(),
        }))
        .await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["remote"], "https://github.com/test/repo");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_repo_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = RepoTools::new(db, ChangeNotifier::new());
    let result = tools
        .get_repo(Parameters(GetRepoParams {
            id: "nonexist".to_string(),
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_repo() {
    let db = setup_db().await;
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/original".to_string(),
        path: Some("/original/path".to_string()),
        tags: vec!["original".to_string()],
        project_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.repos().create(&repo).await.unwrap();
    let db = Arc::new(db);

    let tools = RepoTools::new(db.clone(), ChangeNotifier::new());
    let result = tools
        .update_repo(Parameters(UpdateRepoParams {
            id: "12345678".to_string(),
            remote: Some("https://github.com/test/updated".to_string()),
            path: Some("/updated/path".to_string()),
            tags: Some(vec!["updated".to_string()]),
            project_ids: None,
        }))
        .await;
    assert!(result.is_ok());

    let updated = db.repos().get("12345678").await.unwrap();
    assert_eq!(updated.remote, "https://github.com/test/updated");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_repo_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = RepoTools::new(db, ChangeNotifier::new());
    let result = tools
        .update_repo(Parameters(UpdateRepoParams {
            id: "nonexist".to_string(),
            remote: Some("https://github.com/test/repo".to_string()),
            path: None,
            tags: None,
            project_ids: None,
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_repo() {
    let db = setup_db().await;
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.repos().create(&repo).await.unwrap();
    let db = Arc::new(db);

    let tools = RepoTools::new(db.clone(), ChangeNotifier::new());
    let result = tools
        .delete_repo(Parameters(DeleteRepoParams {
            id: "12345678".to_string(),
        }))
        .await;
    assert!(result.is_ok());

    let result = db.repos().get("12345678").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_repo_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);

    let tools = RepoTools::new(db.clone(), ChangeNotifier::new());
    let result = tools
        .delete_repo(Parameters(DeleteRepoParams {
            id: "nonexist".to_string(),
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_repos_with_data() {
    let db = setup_db().await;
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec!["test".to_string()],
        project_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.repos().create(&repo).await.unwrap();
    let db = Arc::new(db);

    let tools = RepoTools::new(db, ChangeNotifier::new());
    let result = tools
        .list_repos(Parameters(ListReposParams {
            query: None,
            project_id: None,
            limit: None,
            offset: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_repos_pagination() {
    let db = setup_db().await;
    for i in 0..3 {
        let repo = Repo {
            id: format!("repo{:04}", i),
            remote: format!("https://github.com/test/repo{}", i),
            path: None,
            tags: vec![],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
        };
        db.repos().create(&repo).await.unwrap();
    }
    let db = Arc::new(db);

    let tools = RepoTools::new(db, ChangeNotifier::new());
    let result = tools
        .list_repos(Parameters(ListReposParams {
            query: None,
            project_id: None,
            limit: Some(1),
            offset: None,
            sort: None,
            order: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["items"].as_array().unwrap().len(), 1);
    assert_eq!(response["total"], 3);
}
