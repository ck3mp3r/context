//! Tests for Skill MCP tools

use crate::api::notifier::ChangeNotifier;
use crate::db::{Database, Skill, SkillRepository, SqliteDatabase};
use crate::mcp::tools::skills::{GetSkillParams, ListSkillsParams, SearchSkillsParams, SkillTools};
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
async fn test_search_skills_single_match() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());

    // Create two skills
    let rust = Skill {
        id: String::new(),
        name: "rust".to_string(),
        description: "Systems programming language".to_string(),
        content: r#"---
name: rust
description: Systems programming language
---

# Rust Programming

Learn web programming with Rust.
"#
        .to_string(),
        tags: vec!["lang".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: None,
        updated_at: None,
    };
    let python = Skill {
        id: String::new(),
        name: "python".to_string(),
        description: "High-level programming".to_string(),
        content: r#"---
name: python
description: High-level programming
---

# Python Programming

Learn web programming with Python.
"#
        .to_string(),
        tags: vec!["lang".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
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
        description: "Async programming in Rust".to_string(),
        content: r#"---
name: rust-async
description: Async programming in Rust
---

# Async Rust

Learn web programming with async Rust.
"#
        .to_string(),
        tags: vec!["rust".to_string(), "async".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: None,
        updated_at: None,
    };
    let basics_skill = Skill {
        id: String::new(),
        name: "rust-basics".to_string(),
        description: "Basic Rust syntax and types".to_string(),
        content: r#"---
name: rust-basics
description: Basic Rust syntax and types
---

# Rust Basics

Learn web programming basics in Rust.
"#
        .to_string(),
        tags: vec!["rust".to_string(), "basics".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
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
        description: "For work projects".to_string(),
        content: r#"---
name: work-skill
description: For work projects
---

# Work Skill

Learn web programming for work.
"#
        .to_string(),
        tags: vec!["work".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: None,
        updated_at: None,
    };
    let skill2 = Skill {
        id: String::new(),
        name: "personal-skill".to_string(),
        description: "For personal projects".to_string(),
        content: r#"---
name: personal-skill
description: For personal projects
---

# Personal Skill

Learn web programming for personal projects.
"#
        .to_string(),
        tags: vec!["personal".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
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
        description: "First skill".to_string(),
        content: r#"---
name: alpha-skill
description: First skill
---

# Alpha Skill

Learn web programming.
"#
        .to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 10:00:00".to_string()),
        updated_at: Some("2025-01-01 10:00:00".to_string()),
    };

    let skill2 = Skill {
        id: String::new(),
        name: "beta-skill".to_string(),
        description: "Second skill".to_string(),
        content: r#"---
name: beta-skill
description: Second skill
---

# Beta Skill

Learn web programming.
"#
        .to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-02 10:00:00".to_string()),
        updated_at: Some("2025-01-02 10:00:00".to_string()),
    };

    let skill3 = Skill {
        id: String::new(),
        name: "gamma-skill".to_string(),
        description: "Third skill".to_string(),
        content: r#"---
name: gamma-skill
description: Third skill
---

# Gamma Skill

Learn web programming.
"#
        .to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
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

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_with_cache_extraction() {
    use base64::Engine as _;

    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a skill
    let skill = Skill {
        id: "skl00001".to_string(),
        name: "test-skill".to_string(),
        description: "Test skill with attachments".to_string(),
        content: r#"---
name: test-skill
description: Test skill with attachments
---

# Test Skill

Test instructions for skill with attachments.
"#
        .to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: None,
        updated_at: None,
    };
    db.skills().create(&skill).await.unwrap();

    // Create an attachment directly using SQL (since create_attachment is internal)
    let content_base64 = base64::prelude::BASE64_STANDARD.encode(b"#!/bin/bash\necho 'deploying'");
    sqlx::query(
        "INSERT INTO skill_attachment (id, skill_id, type, filename, content, content_hash, mime_type, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind("att00001")
    .bind("skl00001")
    .bind("script")
    .bind("scripts/deploy.sh") // Use relative path like real scanner
    .bind(&content_base64)
    .bind("abc123")
    .bind("text/x-shellscript")
    .bind("2025-01-01 00:00:00")
    .bind("2025-01-01 00:00:00")
    .execute(db.pool())
    .await
    .unwrap();

    // Call MCP tool
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());
    let params = GetSkillParams {
        skill_id: "skl00001".to_string(),
    };

    let result = tools
        .get_skill(Parameters(params))
        .await
        .expect("get_skill should succeed");

    // Parse response
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    // Verify skill data
    assert_eq!(json["id"], "skl00001");
    assert_eq!(json["name"], "test-skill");
    assert_eq!(
        json["scripts"].as_array().unwrap(),
        &vec!["scripts/deploy.sh"]
    ); // Full relative path

    // Verify cache_path exists and points to valid directory
    let cache_path = json["cache_path"]
        .as_str()
        .expect("cache_path should be present");
    let cache_dir = std::path::Path::new(cache_path);
    assert!(cache_dir.exists(), "Cache directory should exist");
    assert!(
        cache_dir.join("scripts/deploy.sh").exists(),
        "Script should exist in cache"
    );

    // Clean up cache for this test
    let _ = std::fs::remove_dir_all(cache_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_skill_without_attachments_no_cache() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    let db = Arc::new(db);

    // Create a skill WITHOUT attachments
    let skill = Skill {
        id: "skl00002".to_string(),
        name: "no-attachments".to_string(),
        description: "Skill without attachments".to_string(),
        content: r#"---
name: no-attachments
description: Skill without attachments
---

# No Attachments

Test instructions for skill without attachments.
"#
        .to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: None,
        updated_at: None,
    };
    db.skills().create(&skill).await.unwrap();

    // Call MCP tool
    let tools = SkillTools::new(db.clone(), ChangeNotifier::new());
    let params = GetSkillParams {
        skill_id: "skl00002".to_string(),
    };

    let result = tools
        .get_skill(Parameters(params))
        .await
        .expect("get_skill should succeed");

    // Parse response
    let content_text = match &result.content[0].raw {
        RawContent::Text(text) => text.text.as_str(),
        _ => panic!("Expected text content"),
    };
    let json: serde_json::Value = serde_json::from_str(content_text).unwrap();

    // Verify skill data
    assert_eq!(json["id"], "skl00002");
    assert_eq!(json["name"], "no-attachments");

    // cache_path should be null since there are no attachments
    assert!(json["cache_path"].is_null(), "cache_path should be null");
}
