//! Tests for Skill MCP tools

use crate::api::notifier::ChangeNotifier;
use crate::db::{Database, Skill, SkillRepository, SqliteDatabase};
use crate::mcp::tools::skills::{
    CreateSkillParams, DeleteSkillParams, GetSkillParams, ListSkillsParams, SearchSkillsParams,
    SkillTools, UpdateSkillParams,
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

    // Parse JSON response
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    // Empty database should have 0 skills
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
        name: "rust-programming".to_string(),
        description: Some("Systems programming with Rust".to_string()),
        instructions: Some("Focus on async/await and error handling".to_string()),
        tags: Some(vec!["programming".to_string(), "rust".to_string()]),
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
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

    assert_eq!(created.name, "rust-programming");
    assert_eq!(
        created.description,
        Some("Systems programming with Rust".to_string())
    );
    assert_eq!(
        created.tags,
        vec!["programming".to_string(), "rust".to_string()]
    );
    assert!(!created.id.is_empty());

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
    let fetched: Skill = serde_json::from_str(content_text).unwrap();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "rust-programming");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_not_found() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    let params = GetSkillParams {
        skill_id: "nonexistent".to_string(),
    };

    let result = tools.get_skill(Parameters(params)).await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_skill() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    // Create skill
    let skill = Skill {
        id: String::new(),
        name: "python".to_string(),
        description: Some("Python programming".to_string()),
        instructions: Some("Learn Python fundamentals".to_string()),
        tags: vec!["lang".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    let created = db.skills().create(&skill).await.unwrap();

    // Update skill
    let update_params = UpdateSkillParams {
        skill_id: created.id.clone(),
        name: Some("advanced-python".to_string()),
        description: Some("Advanced Python programming".to_string()),
        instructions: Some("Focus on asyncio and type hints".to_string()),
        tags: Some(vec!["lang".to_string(), "advanced".to_string()]),
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
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

    assert_eq!(updated.name, "advanced-python");
    assert_eq!(
        updated.description,
        Some("Advanced Python programming".to_string())
    );
    assert_eq!(
        updated.instructions,
        Some("Focus on asyncio and type hints".to_string())
    );
    assert_eq!(
        updated.tags,
        vec!["lang".to_string(), "advanced".to_string()]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_skill() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    // Create skill
    let skill = Skill {
        id: String::new(),
        name: "javascript".to_string(),
        description: Some("Web programming".to_string()),
        instructions: Some("Learn web programming".to_string()),
        tags: vec![],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    let created = db.skills().create(&skill).await.unwrap();

    // Delete skill
    let delete_params = DeleteSkillParams {
        skill_id: created.id.clone(),
    };

    let result = tools
        .delete_skill(Parameters(delete_params))
        .await
        .expect("delete should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    assert!(content_text.contains("deleted successfully"));

    // Verify it's deleted
    let get_result = db.skills().get(&created.id).await;
    assert!(get_result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_skills_single_match() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    // Create two skills
    let rust = Skill {
        id: String::new(),
        name: "rust".to_string(),
        description: Some("Systems programming language".to_string()),
        instructions: Some("Learn web programming".to_string()),
        tags: vec!["lang".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    let python = Skill {
        id: String::new(),
        name: "python".to_string(),
        description: Some("High-level programming".to_string()),
        instructions: Some("Learn web programming".to_string()),
        tags: vec!["lang".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.skills().create(&rust).await.unwrap();
    db.skills().create(&python).await.unwrap();

    // Search for "Rust"
    let params = SearchSkillsParams {
        query: "rust".to_string(),
        tags: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };
    let result = tools
        .search_skills(Parameters(params))
        .await
        .expect("search_skills should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["name"], "rust");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_skills_with_tag_filter() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    // Create skills
    let async_skill = Skill {
        id: String::new(),
        name: "rust-async".to_string(),
        description: Some("Async programming in Rust".to_string()),
        instructions: Some("Learn web programming".to_string()),
        tags: vec!["rust".to_string(), "async".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    let basics_skill = Skill {
        id: String::new(),
        name: "rust-basics".to_string(),
        description: Some("Basic Rust syntax and types".to_string()),
        instructions: Some("Learn web programming".to_string()),
        tags: vec!["rust".to_string(), "basics".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.skills().create(&async_skill).await.unwrap();
    db.skills().create(&basics_skill).await.unwrap();

    // Search for "Rust" with "async" tag filter
    let params = SearchSkillsParams {
        query: "rust".to_string(),
        tags: Some(vec!["async".to_string()]),
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };
    let result = tools
        .search_skills(Parameters(params))
        .await
        .expect("search_skills should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["name"], "rust-async");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_search_skills_empty_results() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    // Search for a non-existent string
    let params = SearchSkillsParams {
        query: "Nonexistent".to_string(),
        tags: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };
    let result = tools
        .search_skills(Parameters(params))
        .await
        .expect("search_skills should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 0);
    assert_eq!(json["items"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_with_tag_filter() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create skills with different tags
    let skill1 = Skill {
        id: String::new(),
        name: "work-skill".to_string(),
        description: Some("For work projects".to_string()),
        instructions: Some("Learn web programming".to_string()),
        tags: vec!["work".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    let skill2 = Skill {
        id: String::new(),
        name: "personal-skill".to_string(),
        description: Some("For personal projects".to_string()),
        instructions: Some("Learn web programming".to_string()),
        tags: vec!["personal".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.skills().create(&skill1).await.unwrap();
    db.skills().create(&skill2).await.unwrap();

    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    // List only "work" skills
    let params = ListSkillsParams {
        tags: Some(vec!["work".to_string()]),
        project_id: None,
        limit: None,
        offset: None,
        sort: None,
        order: None,
    };

    let result = tools
        .list_skills(Parameters(params))
        .await
        .expect("list should succeed");

    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    assert_eq!(json["total"], 1);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items[0]["name"], "work-skill");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_skills_with_sort_and_order() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create skills with specific timestamps for sorting
    let skill1 = Skill {
        id: String::new(),
        name: "alpha-skill".to_string(),
        description: Some("First skill".to_string()),
        instructions: Some("Learn web programming".to_string()),
        tags: vec![],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: Some("2025-01-01 10:00:00".to_string()),
        updated_at: Some("2025-01-01 10:00:00".to_string()),
    };

    let skill2 = Skill {
        id: String::new(),
        name: "beta-skill".to_string(),
        description: Some("Second skill".to_string()),
        instructions: Some("Learn web programming".to_string()),
        tags: vec![],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: Some("2025-01-02 10:00:00".to_string()),
        updated_at: Some("2025-01-02 10:00:00".to_string()),
    };

    let skill3 = Skill {
        id: String::new(),
        name: "gamma-skill".to_string(),
        description: Some("Third skill".to_string()),
        instructions: Some("Learn web programming".to_string()),
        tags: vec![],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        created_at: Some("2025-01-03 10:00:00".to_string()),
        updated_at: Some("2025-01-03 10:00:00".to_string()),
    };

    db.skills().create(&skill1).await.unwrap();
    db.skills().create(&skill2).await.unwrap();
    db.skills().create(&skill3).await.unwrap();

    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    // Test sorting by created_at DESC
    let params = ListSkillsParams {
        tags: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: Some("created_at".to_string()),
        order: Some("desc".to_string()),
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

    assert_eq!(json["total"], 3);
    let items = json["items"].as_array().unwrap();
    // Should be ordered by created_at DESC: skill3, skill2, skill1
    assert_eq!(items[0]["name"], "gamma-skill");
    assert_eq!(items[1]["name"], "beta-skill");
    assert_eq!(items[2]["name"], "alpha-skill");

    // Test sorting by name ASC
    let params = ListSkillsParams {
        tags: None,
        project_id: None,
        limit: None,
        offset: None,
        sort: Some("name".to_string()),
        order: Some("asc".to_string()),
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

    assert_eq!(json["total"], 3);
    let items = json["items"].as_array().unwrap();
    // Should be ordered by name ASC
    assert_eq!(items[0]["name"], "alpha-skill");
    assert_eq!(items[1]["name"], "beta-skill");
    assert_eq!(items[2]["name"], "gamma-skill");
}

// --- Agent Skills Specification Tests ---

#[tokio::test(flavor = "multi_thread")]
async fn test_create_skill_with_agent_skills_fields() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    let create_params = CreateSkillParams {
        name: "agent-skill".to_string(),
        description: Some("A skill with Agent Skills metadata".to_string()),
        instructions: Some("Follow these steps".to_string()),
        tags: Some(vec!["agent".to_string(), "spec".to_string()]),
        license: Some("MIT".to_string()),
        compatibility: Some("opencode>=0.1.0".to_string()),
        allowed_tools: Some(vec!["read".to_string(), "write".to_string()]),
        metadata: Some(serde_json::json!({"author": "test"})),
        origin_url: Some("https://github.com/example/skill".to_string()),
        origin_ref: Some("main".to_string()),
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

    assert_eq!(created.name, "agent-skill");
    assert_eq!(created.license, Some("MIT".to_string()));
    assert_eq!(created.compatibility, Some("opencode>=0.1.0".to_string()));
}
