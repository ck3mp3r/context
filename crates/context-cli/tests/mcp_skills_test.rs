//! Tests for Skill MCP tools
//!
//! These tests were moved from context-server (Phase 6) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use base64::Engine;
use context_core::get_data_dir;
use context_core::{HasProjects, HasSkills, Project, ProjectRepository, Skill, SkillRepository};
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

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_rejects_name_lookup() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: "---\nname: test-skill\n---\n# Test\n\nTest content".to_string(),
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

    // get_skill only supports ID lookup, not name lookup
    let params = GetSkillParams {
        skill_id: "test-skill".to_string(), // This is a name, not an ID
    };

    let result = tools.get_skill(Parameters(params)).await;
    assert!(result.is_err(), "get_skill should reject name lookup");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: "---\nname: test-skill\n---\n# Test\n\nTest content".to_string(),
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

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_extracts_attachments_to_cache() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: "---\nname: test-skill\n---\n# Test\n\nTest content".to_string(),
        tags: vec!["test".to_string()],
        project_ids: vec![],
        scripts: vec!["script.sh".to_string()],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.skills().create(&skill).await.unwrap();

    // Add an attachment
    let attachment = context_core::SkillAttachment {
        id: "attch001".to_string(),
        skill_id: "skill001".to_string(),
        type_: "script".to_string(),
        filename: "script.sh".to_string(),
        content: base64::prelude::BASE64_STANDARD.encode(b"echo hello"),
        content_hash: "abc123".to_string(),
        mime_type: Some("text/x-shellscript".to_string()),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.skills().create_attachment(&attachment).await.unwrap();

    let db = Arc::new(db);
    let skills_dir = std::env::temp_dir().join(format!("c5t-test-skills-{}", std::process::id()));
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new(), skills_dir.clone());

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

    // Verify cache_path is returned
    let cache_path = response["cache_path"].as_str();
    assert!(
        cache_path.is_some(),
        "cache_path should be present when skill has attachments"
    );

    // Verify cache directory exists
    let cache_dir = std::path::Path::new(cache_path.unwrap());
    assert!(cache_dir.exists(), "Cache directory should exist");
    assert!(
        cache_dir.join("SKILL.md").exists(),
        "SKILL.md should be in cache"
    );
    assert!(
        cache_dir.join("script.sh").exists(),
        "script.sh should be in cache"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&skills_dir);
}

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

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_only_changes_tags_and_project_ids() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "Original description".to_string(),
        content: "---\nname: test-skill\n---\n# Original\n\nContent".to_string(),
        tags: vec!["original".to_string()],
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

    // Update only tags and project_ids
    let result = tools
        .update_skill(Parameters(UpdateSkillParams {
            skill_id: "skill001".to_string(),
            tags: Some(vec!["updated".to_string()]),
            project_ids: Some(vec![]),
        }))
        .await;
    assert!(result.is_ok());

    // Verify content and description are unchanged
    let updated = db.skills().get("skill001").await.unwrap();
    assert_eq!(updated.tags, vec!["updated"], "Tags should be updated");
    assert_eq!(
        updated.description, "Original description",
        "Description should be unchanged"
    );
    assert_eq!(
        updated.content, "---\nname: test-skill\n---\n# Original\n\nContent",
        "Content should be unchanged"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_with_data() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: "---\nname: test-skill\n---\n# Test\n\nTest content".to_string(),
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
        content: "---\nname: rust-async\n---\n# Rust Async\n\nAsync patterns".to_string(),
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
            content: format!("---\nname: skill-{}\n---\n# Skill {}\n\nContent", i, i),
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
        content: "---\nname: test-skill\n---\n# Test\n\nContent".to_string(),
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

    // Create the project first to satisfy FK constraint
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.projects().create(&project).await.unwrap();

    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: "---\nname: test-skill\n---\n# Test\n\nContent".to_string(),
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
        content: "---\nname: test-skill\n---\n# Original\n\nContent".to_string(),
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

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill_invalidates_cache() {
    let db = setup_db().await;
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: "---\nname: test-skill\n---\n# Test\n\nTest content".to_string(),
        tags: vec!["test".to_string()],
        project_ids: vec![],
        scripts: vec!["script.sh".to_string()],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.skills().create(&skill).await.unwrap();

    // Add an attachment
    let attachment = context_core::SkillAttachment {
        id: "attch001".to_string(),
        skill_id: "skill001".to_string(),
        type_: "script".to_string(),
        filename: "script.sh".to_string(),
        content: base64::prelude::BASE64_STANDARD.encode(b"echo hello"),
        content_hash: "abc123".to_string(),
        mime_type: Some("text/x-shellscript".to_string()),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    db.skills().create_attachment(&attachment).await.unwrap();

    let db = Arc::new(db);
    let skills_dir = get_data_dir().join("skills");
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new(), skills_dir.clone());

    // First, call get_skill to populate the cache
    let get_params = GetSkillParams {
        skill_id: "skill001".to_string(),
    };
    let get_result = tools.get_skill(Parameters(get_params)).await;
    assert!(get_result.is_ok());

    let call_result = get_result.unwrap();
    let content_text = match &call_result.content[0] {
        ContentBlock::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let response: serde_json::Value = serde_json::from_str(content_text).unwrap();
    let cache_path = response["cache_path"].as_str().unwrap();
    let cache_dir = std::path::Path::new(cache_path);
    assert!(cache_dir.exists(), "Cache should exist after get_skill");

    // Now update the skill - this should invalidate the cache
    let update_result = tools
        .update_skill(Parameters(UpdateSkillParams {
            skill_id: "skill001".to_string(),
            tags: Some(vec!["updated".to_string()]),
            project_ids: Some(vec![]),
        }))
        .await;
    assert!(update_result.is_ok());

    // Verify cache is invalidated (directory removed)
    assert!(
        !cache_dir.exists(),
        "Cache should be invalidated after update"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&skills_dir);
}
