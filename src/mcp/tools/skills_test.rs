//! Tests for Skill MCP tools

use crate::api::notifier::ChangeNotifier;
use crate::db::{Database, Skill, SkillRepository, SqliteDatabase};
use crate::mcp::tools::skills::{
    CreateSkillParams, DeleteSkillParams, GetSkillParams, ListSkillsParams, SkillTools,
    UpdateSkillParams,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::RawContent;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_empty() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    let params = ListSkillsParams {
        tags: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_skills(Parameters(params))
        .await
        .expect("list_skills should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 0);
    assert_eq!(json["items"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_and_get_skill() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    // Create skill
    let create_params = CreateSkillParams {
        name: "Rust".to_string(),
        description: Some("Systems programming".to_string()),
        instructions: Some("Follow the book".to_string()),
        tags: Some(vec!["lang".to_string(), "systems".to_string()]),
        project_ids: None,
    };

    let result = tools
        .create_skill(Parameters(create_params))
        .await
        .expect("create should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let created: Skill = serde_json::from_str(content_text).unwrap();

    assert_eq!(created.name, "Rust");
    assert!(created.id.starts_with("skl") || !created.id.is_empty());

    // Get the skill
    let get_params = GetSkillParams {
        skill_id: created.id.clone(),
    };

    let result = tools
        .get_skill(Parameters(get_params))
        .await
        .expect("get should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let retrieved: Skill = serde_json::from_str(content_text).unwrap();

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.name, "Rust");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_not_found() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    let params = GetSkillParams {
        skill_id: "skl_NONEXISTENT".to_string(),
    };

    let result = tools.get_skill(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_and_delete() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    // Create
    let create_params = CreateSkillParams {
        name: "ModMe".to_string(),
        description: None,
        instructions: None,
        tags: None,
        project_ids: None,
    };

    let result = tools
        .create_skill(Parameters(create_params))
        .await
        .expect("create should succeed");
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let created: Skill = serde_json::from_str(content_text).unwrap();

    // Update
    let update_params = UpdateSkillParams {
        skill_id: created.id.clone(),
        name: Some("Updated".to_string()),
        description: Some("New desc".to_string()),
        instructions: None,
        tags: Some(vec!["updated".to_string()]),
        project_ids: None,
    };

    let result = tools
        .update_skill(Parameters(update_params))
        .await
        .expect("update should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let updated: Skill = serde_json::from_str(content_text).unwrap();
    assert_eq!(updated.name, "Updated");
    assert_eq!(updated.tags, vec!["updated".to_string()]);

    // Delete
    let delete_params = DeleteSkillParams {
        skill_id: created.id.clone(),
    };
    let _ = tools
        .delete_skill(Parameters(delete_params))
        .await
        .expect("delete should succeed");

    // Confirm deleted
    let get_params = GetSkillParams {
        skill_id: created.id.clone(),
    };
    let result = tools.get_skill(Parameters(get_params)).await;
    assert!(result.is_err());
}
