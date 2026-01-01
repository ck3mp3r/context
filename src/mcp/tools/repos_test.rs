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
        .list_repos(Parameters(ListReposParams {
            project_id: None,
            limit: None,
            sort: None,
            order: None,
        }))
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
            project_ids: None,
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
async fn test_create_repo_with_project_ids() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a project first
    use crate::db::{Project, ProjectRepository};
    let project = Project {
        id: "proj9999".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    let tools = RepoTools::new(Arc::clone(&db));
    let result = tools
        .create_repo(Parameters(CreateRepoParams {
            remote: "git@github.com:user/linked.git".to_string(),
            path: Some("/path/to/linked".to_string()),
            tags: None,
            project_ids: Some(vec!["proj9999".to_string()]),
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
    assert_eq!(repo_json["remote"], "git@github.com:user/linked.git");
    assert_eq!(
        repo_json["project_ids"].as_array().unwrap(),
        &vec![serde_json::json!("proj9999")]
    );
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
            project_ids: None,
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
async fn test_update_repo_with_project_ids() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a project first
    use crate::db::{Project, ProjectRepository};
    let project = Project {
        id: "proj1234".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    // Create initial repo
    let repo = Repo {
        id: "repo5678".to_string(),
        remote: "git@github.com:user/test.git".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec![],
        project_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
    };
    db.repos().create(&repo).await.unwrap();

    let tools = RepoTools::new(Arc::clone(&db));
    let result = tools
        .update_repo(Parameters(UpdateRepoParams {
            id: "repo5678".to_string(),
            remote: None,
            path: None,
            tags: None,
            project_ids: Some(vec!["proj1234".to_string()]),
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
    assert_eq!(repo_json["id"], "repo5678");
    assert_eq!(
        repo_json["project_ids"].as_array().unwrap(),
        &vec![serde_json::json!("proj1234")]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_repo_transaction_safety() {
    // This test verifies that if updating project_ids fails midway,
    // the entire update is rolled back (transaction safety)
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create two projects
    use crate::db::{Project, ProjectRepository};
    let project1 = Project {
        id: "proj1111".to_string(),
        title: "Project 1".to_string(),
        description: None,
        tags: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    db.projects().create(&project1).await.unwrap();

    // Create repo linked to project1
    let repo = Repo {
        id: "repotxn1".to_string(),
        remote: "git@github.com:test/txn.git".to_string(),
        path: Some("/test".to_string()),
        tags: vec![],
        project_ids: vec!["proj1111".to_string()],
        created_at: "2025-01-01 00:00:00".to_string(),
    };
    db.repos().create(&repo).await.unwrap();

    // Verify initial state
    let loaded = db.repos().get("repotxn1").await.unwrap();
    assert_eq!(loaded.project_ids, vec!["proj1111"]);

    // Update with invalid project ID (should fail)
    let mut updated_repo = repo.clone();
    updated_repo.project_ids = vec!["invalid9".to_string()];
    let result = db.repos().update(&updated_repo).await;

    // Should fail due to foreign key constraint
    assert!(result.is_err());

    // Verify repo state is unchanged (transaction rolled back)
    let still_original = db.repos().get("repotxn1").await.unwrap();
    assert_eq!(still_original.project_ids, vec!["proj1111"]);
    assert_eq!(still_original.remote, "git@github.com:test/txn.git");
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
        .list_repos(Parameters(ListReposParams {
            project_id: None,
            limit: None,
            sort: None,
            order: None,
        }))
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
        .list_repos(Parameters(ListReposParams {
            project_id: None,
            limit: Some(5),
            sort: None,
            order: None,
        }))
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
        .list_repos(Parameters(ListReposParams {
            project_id: None,
            limit: Some(50),
            sort: None,
            order: None,
        }))
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

#[tokio::test(flavor = "multi_thread")]
async fn test_list_repos_with_sort_and_order() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create repos with specific data for sorting
    let repo1 = Repo {
        id: String::new(),
        remote: "git@github.com:zzz/repo.git".to_string(),
        path: Some("/aaa/path".to_string()),
        tags: vec![],
        project_ids: vec![],
        created_at: "2025-01-01 10:00:00".to_string(),
    };

    let repo2 = Repo {
        id: String::new(),
        remote: "git@github.com:aaa/repo.git".to_string(),
        path: Some("/zzz/path".to_string()),
        tags: vec![],
        project_ids: vec![],
        created_at: "2025-01-03 10:00:00".to_string(),
    };

    let repo3 = Repo {
        id: String::new(),
        remote: "git@github.com:mmm/repo.git".to_string(),
        path: Some("/mmm/path".to_string()),
        tags: vec![],
        project_ids: vec![],
        created_at: "2025-01-02 10:00:00".to_string(),
    };

    db.repos().create(&repo1).await.unwrap();
    db.repos().create(&repo2).await.unwrap();
    db.repos().create(&repo3).await.unwrap();

    let tools = RepoTools::new(db);

    // Test sorting by remote ASC
    let result = tools
        .list_repos(Parameters(ListReposParams {
            project_id: None,
            limit: None,
            sort: Some("remote".to_string()),
            order: Some("asc".to_string()),
        }))
        .await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let repos: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();

    assert_eq!(repos.len(), 3);
    // Should be sorted by remote ASC: aaa, mmm, zzz
    assert_eq!(repos[0]["remote"], "git@github.com:aaa/repo.git");
    assert_eq!(repos[1]["remote"], "git@github.com:mmm/repo.git");
    assert_eq!(repos[2]["remote"], "git@github.com:zzz/repo.git");

    // Test sorting by created_at DESC
    // Note: created_at is auto-generated by the repo, so we can't control exact values
    // But we can verify the order is correct (DESC means most recent first)
    let result = tools
        .list_repos(Parameters(ListReposParams {
            project_id: None,
            limit: None,
            sort: Some("created_at".to_string()),
            order: Some("desc".to_string()),
        }))
        .await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let repos: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();

    assert_eq!(repos.len(), 3);
    // Verify order is DESC by comparing timestamps
    let ts0 = repos[0]["created_at"].as_str().unwrap();
    let ts1 = repos[1]["created_at"].as_str().unwrap();
    let ts2 = repos[2]["created_at"].as_str().unwrap();
    assert!(ts0 >= ts1, "First timestamp should be >= second");
    assert!(ts1 >= ts2, "Second timestamp should be >= third");

    // Test sorting by path DESC
    let result = tools
        .list_repos(Parameters(ListReposParams {
            project_id: None,
            limit: None,
            sort: Some("path".to_string()),
            order: Some("desc".to_string()),
        }))
        .await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let repos: Vec<serde_json::Value> = serde_json::from_str(content_text).unwrap();

    assert_eq!(repos.len(), 3);
    // Should be sorted by path DESC: zzz, mmm, aaa
    assert_eq!(repos[0]["path"], "/zzz/path");
    assert_eq!(repos[1]["path"], "/mmm/path");
    assert_eq!(repos[2]["path"], "/aaa/path");
}
