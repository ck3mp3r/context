//! Tests for Repository MCP tools

use crate::db::{Database, Repo, RepoRepository, SqliteDatabase};
use crate::mcp::tools::repos::*;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, RawContent},
};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_repos_empty() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let tools = RepoTools::new(db);
    let result = tools
        .list_repos(Parameters(ListReposParams { limit: None }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let repos: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();
    assert_eq!(repos.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_repo() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a test repo
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "git@github.com:user/repo.git".to_string(),
        path: Some("/path/to/repo".to_string()),
        tags: vec![],
        project_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
    };
    db.repos().create(&repo).await.unwrap();

    let tools = RepoTools::new(Arc::clone(&db));
    let result = tools
        .get_repo(Parameters(GetRepoParams {
            id: "12345678".to_string(),
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let repo_json: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(repo_json["id"], "12345678");
    assert_eq!(repo_json["remote"], "git@github.com:user/repo.git");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_repo() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    let tools = RepoTools::new(Arc::clone(&db));
    let result = tools
        .create_repo(Parameters(CreateRepoParams {
            remote: "git@github.com:user/new.git".to_string(),
            path: Some("/path/to/new".to_string()),
            tags: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let repo_json: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(repo_json["remote"], "git@github.com:user/new.git");
    assert!(repo_json["id"].as_str().unwrap().len() == 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_repo() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create initial repo
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "git@github.com:user/old.git".to_string(),
        path: Some("/old/path".to_string()),
        tags: vec![],
        project_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
    };
    db.repos().create(&repo).await.unwrap();

    let tools = RepoTools::new(Arc::clone(&db));
    let result = tools
        .update_repo(Parameters(UpdateRepoParams {
            id: "12345678".to_string(),
            remote: Some("git@github.com:user/updated.git".to_string()),
            path: Some("/new/path".to_string()),
            tags: None,
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };

    let repo_json: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(repo_json["remote"], "git@github.com:user/updated.git");
    assert_eq!(repo_json["path"], "/new/path");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_repo() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a repo to delete
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "git@github.com:user/todelete.git".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
    };
    db.repos().create(&repo).await.unwrap();

    let tools = RepoTools::new(Arc::clone(&db));
    let result = tools
        .delete_repo(Parameters(DeleteRepoParams {
            id: "12345678".to_string(),
        }))
        .await;
    assert!(result.is_ok());

    let call_result: CallToolResult = result.unwrap();
    assert!(call_result.is_error.is_none() || call_result.is_error == Some(false));

    // Verify it's deleted
    let get_result = db.repos().get("12345678").await;
    assert!(get_result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_repos_respects_limit() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create 25 test repos
    for i in 0..25 {
        let repo = Repo {
            id: format!("repo{:04}", i),
            remote: format!("git@github.com:user/repo{}.git", i),
            path: None,
            tags: vec![],
            project_ids: vec![],
            created_at: "2025-01-01 00:00:00".to_string(),
        };
        db.repos().create(&repo).await.unwrap();
    }

    let tools = RepoTools::new(Arc::clone(&db));

    // Test 1: Without limit parameter, should return DEFAULT_LIMIT (10)
    let result = tools
        .list_repos(Parameters(ListReposParams { limit: None }))
        .await;
    assert!(result.is_ok());
    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let repos: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();
    assert_eq!(repos.len(), 10, "Should return DEFAULT_LIMIT (10) items");

    // Test 2: With limit=5, should return 5
    let result = tools
        .list_repos(Parameters(ListReposParams { limit: Some(5) }))
        .await;
    assert!(result.is_ok());
    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let repos: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();
    assert_eq!(repos.len(), 5, "Should return requested 5 items");

    // Test 3: With limit=50 (exceeds MAX_LIMIT), should cap at MAX_LIMIT (20)
    let result = tools
        .list_repos(Parameters(ListReposParams { limit: Some(50) }))
        .await;
    assert!(result.is_ok());
    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let repos: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();
    assert_eq!(
        repos.len(),
        20,
        "Should cap at MAX_LIMIT (20) even though 50 requested"
    );
}
