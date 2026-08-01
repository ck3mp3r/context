//! Tests for Skill MCP tools
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use context_core::get_data_dir;
use context_core::{Database, HasSkills, Skill, SkillRepository};
use context_db::SqliteDatabase;
use context_server::api::notifier::ChangeNotifier;
use context_server::mcp::tools::skills::{
    GetSkillParams, ListSkillsParams, SkillTools, UpdateSkillParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ContentBlock;
use std::sync::Arc;

async fn setup_db() -> SqliteDatabase {
    common::setup_db().await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_empty() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = SkillTools::new(
        db.clone(),
        ChangeNotifier::new(),
        get_data_dir().join("skills"),
    );

    let params = ListSkillsParams {
        query: None,
        tags: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_skills(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["total"], 0);
}

// TODO: API changed - create_skill method removed. Skills are created via
// the skills import mechanism, not via MCP tools.
// #[tokio::test(flavor = "multi_thread")]
// async fn test_create_skill() { ... }

// TODO: API changed - create_skill method removed.
// #[tokio::test(flavor = "multi_thread")]
// async fn test_create_skill_empty_name() { ... }

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: "# Test\n\nTest content".to_string(),
        tags: vec!["test".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.skills().create(&skill).await.unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(
        db.clone(),
        ChangeNotifier::new(),
        get_data_dir().join("skills"),
    );

    let params = GetSkillParams {
        skill_id: "skill001".to_string(),
    };

    let result = tools.get_skill(Parameters(params)).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    assert_eq!(response["name"], "test-skill");
}

// TODO: API changed - get_skill no longer supports lookup by name, only by ID.
// #[tokio::test(flavor = "multi_thread")]
// async fn test_get_skill_by_name() { ... }

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = SkillTools::new(
        db.clone(),
        ChangeNotifier::new(),
        get_data_dir().join("skills"),
    );

    let params = GetSkillParams {
        skill_id: "nonexist".to_string(),
    };

    let result = tools.get_skill(Parameters(params)).await;
    assert!(result.is_err());
}

// TODO: API changed - delete_skill method removed.
// #[tokio::test(flavor = "multi_thread")]
// async fn test_delete_skill() { ... }

// TODO: API changed - delete_skill method removed.
// #[tokio::test(flavor = "multi_thread")]
// async fn test_delete_skill_not_found() { ... }

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_with_data() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: "# Test\n\nTest content".to_string(),
        tags: vec!["test".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.skills().create(&skill).await.unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(
        db.clone(),
        ChangeNotifier::new(),
        get_data_dir().join("skills"),
    );

    let params = ListSkillsParams {
        query: None,
        tags: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_skills(Parameters(params)).await;
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
async fn test_list_skills_search() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "rust-async".to_string(),
        description: "Rust async patterns".to_string(),
        content: "# Rust Async\n\nAsync patterns".to_string(),
        tags: vec!["rust".to_string(), "async".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.skills().create(&skill).await.unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(
        db.clone(),
        ChangeNotifier::new(),
        get_data_dir().join("skills"),
    );

    let params = ListSkillsParams {
        query: Some("rust".to_string()),
        tags: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_skills(Parameters(params)).await;
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
async fn test_list_skills_pagination() {
    let db = setup_db().await;
    for i in 0..3 {
        let skill = Skill {
            id: format!("skill{:03}", i),
            name: format!("skill-{}", i),
            description: format!("Skill {}", i),
            content: format!("# Skill {}\n\nContent", i),
            tags: vec![],
            project_ids: vec![],
            scripts: vec![],
            references: vec![],
            assets: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
        };
        db.skills().create(&skill).await.unwrap();
    }
    let db = Arc::new(db);
    let tools = SkillTools::new(
        db.clone(),
        ChangeNotifier::new(),
        get_data_dir().join("skills"),
    );

    let params = ListSkillsParams {
        query: None,
        tags: None,
        project_id: None,
        limit: Some(1),
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_skills(Parameters(params)).await;
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

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_tag_filter() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: "# Test\n\nContent".to_string(),
        tags: vec!["backend".to_string(), "rust".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.skills().create(&skill).await.unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(
        db.clone(),
        ChangeNotifier::new(),
        get_data_dir().join("skills"),
    );

    let params = ListSkillsParams {
        query: None,
        tags: Some(vec!["backend".to_string()]),
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_skills(Parameters(params)).await;
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
async fn test_list_skills_project_filter() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: "# Test\n\nContent".to_string(),
        tags: vec![],
        project_ids: vec!["proj0001".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.skills().create(&skill).await.unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(
        db.clone(),
        ChangeNotifier::new(),
        get_data_dir().join("skills"),
    );

    let params = ListSkillsParams {
        query: None,
        tags: None,
        project_id: Some("proj0001".to_string()),
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools.list_skills(Parameters(params)).await;
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
async fn test_update_skill() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "Original".to_string(),
        content: "# Original\n\nContent".to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.skills().create(&skill).await.unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(
        db.clone(),
        ChangeNotifier::new(),
        get_data_dir().join("skills"),
    );

    let result = tools
        .update_skill(Parameters(UpdateSkillParams {
            skill_id: "skill001".to_string(),
            tags: Some(vec!["updated".to_string()]),
            project_ids: Some(vec![]),
        }))
        .await;
    assert!(result.is_ok());

    let updated = db.skills().get("skill001").await.unwrap();
    assert_eq!(updated.tags, vec!["updated"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_not_found() {
    let db = setup_db().await;
    let db = Arc::new(db);
    let tools = SkillTools::new(
        db.clone(),
        ChangeNotifier::new(),
        get_data_dir().join("skills"),
    );

    let result = tools
        .update_skill(Parameters(UpdateSkillParams {
            skill_id: "nonexist".to_string(),
            tags: None,
            project_ids: None,
        }))
        .await;
    assert!(result.is_err());
}

// TODO: API changed - enable_skill method removed.
// #[tokio::test(flavor = "multi_thread")]
// async fn test_enable_skill() { ... }

// TODO: API changed - disable_skill method removed.
// #[tokio::test(flavor = "multi_thread")]
// async fn test_disable_skill() { ... }

// TODO: API changed - replace_skill method removed.
// #[tokio::test(flavor = "multi_thread")]
// async fn test_replace_skill() { ... }

// TODO: API changed - patch_skill method removed.
// #[tokio::test(flavor = "multi_thread")]
// async fn test_patch_skill() { ... }
