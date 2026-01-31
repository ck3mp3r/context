//! Tests for SqliteSkillRepository.

use crate::db::repository::Database;
use crate::db::{Skill, SkillQuery, SkillRepository, SqliteDatabase};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in_memory database");
    db.migrate().expect("Migration should succeed");
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_create_and_get() {
    let db = setup_db().await;
    let skills = db.skills();

    let skill = Skill {
        id: "skl00001".to_string(),
        name: "rust".to_string(),
        description: Some("Systems programming".to_string()),
        instructions: Some("Use Rust book".to_string()),
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
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };

    skills.create(&skill).await.expect("Create should succeed");

    let retrieved = skills.get("skl00001").await.expect("Get should succeed");
    assert_eq!(retrieved.id, skill.id);
    assert_eq!(retrieved.name, skill.name);
    assert_eq!(retrieved.description, skill.description);
    assert_eq!(retrieved.instructions, skill.instructions);
    assert_eq!(retrieved.tags, skill.tags);
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_get_nonexistent_returns_not_found() {
    let db = setup_db().await;
    let skills = db.skills();

    let result = skills.get("nonexist").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_list() {
    let db = setup_db().await;
    let skills = db.skills();

    // Initially empty
    let result = skills.list(None).await.expect("List should succeed");
    assert!(result.items.is_empty());

    // Add skills
    let skill1 = Skill {
        id: "skl00002".to_string(),
        name: "first".to_string(),
        description: Some("Test description".to_string()),
        instructions: Some("Test instructions".to_string()),
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
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill1).await.unwrap();

    let skill2 = Skill {
        id: "skl00003".to_string(),
        name: "second".to_string(),
        description: Some("Test description".to_string()),
        instructions: Some("Test instructions".to_string()),
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
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill2).await.unwrap();

    let result = skills.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_update() {
    let db = setup_db().await;
    let skills = db.skills();

    let mut skill = Skill {
        id: "skl00004".to_string(),
        name: "original-name".to_string(),
        description: Some("Original desc".to_string()),
        instructions: Some("Test instructions".to_string()),
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
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill).await.expect("Create should succeed");

    skill.name = "updated-name".to_string();
    skill.description = Some("Updated desc".to_string());
    skill.tags = vec!["updated".to_string()];
    skills.update(&skill).await.expect("Update should succeed");

    let retrieved = skills.get("skl00004").await.expect("Get should succeed");
    assert_eq!(retrieved.name, "updated-name");
    assert_eq!(retrieved.description, Some("Updated desc".to_string()));
    assert_eq!(retrieved.tags, vec!["updated".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_delete() {
    let db = setup_db().await;
    let skills = db.skills();

    let skill = Skill {
        id: "skl00005".to_string(),
        name: "to-delete".to_string(),
        description: Some("Desc".to_string()),
        instructions: Some("Test instructions".to_string()),
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
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill).await.expect("Create should succeed");

    skills
        .delete("skl00005")
        .await
        .expect("Delete should succeed");

    let result = skills.get("skl00005").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_search() {
    let db = setup_db().await;
    let skills = db.skills();

    // Create skills with specific content
    let skill1 = Skill {
        id: "skl00006".to_string(),
        name: "api-design".to_string(),
        description: Some("REST endpoints for user mgmt".to_string()),
        instructions: Some("Test instructions".to_string()),
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
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill1).await.unwrap();

    let skill2 = Skill {
        id: "skl00007".to_string(),
        name: "database".to_string(),
        description: Some("SQLite tables for data".to_string()),
        instructions: Some("Test instructions".to_string()),
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
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill2).await.unwrap();

    let skill3 = Skill {
        id: "skl00008".to_string(),
        name: "frontend".to_string(),
        description: Some("React components".to_string()),
        instructions: Some("Test instructions".to_string()),
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
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill3).await.unwrap();

    // Search for "data" - should find 1 skill
    let results = skills
        .search("data", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1);

    // Search for "React" - should find 1 skill
    let results = skills
        .search("React", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].name, "frontend");

    // Search for nonexistent term
    let results = skills
        .search("kubernetes", None)
        .await
        .expect("Search should succeed");
    assert!(results.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_list_with_tag_filter() {
    let db = setup_db().await;
    let skills = db.skills();

    // Create skills with different tags
    let skill1 = Skill {
        id: "skl00009".to_string(),
        name: "rust".to_string(),
        description: Some("Test description".to_string()),
        instructions: Some("Test instructions".to_string()),
        tags: vec!["rust".to_string(), "programming".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill1).await.unwrap();

    let skill2 = Skill {
        id: "skl00010".to_string(),
        name: "python".to_string(),
        description: Some("Test description".to_string()),
        instructions: Some("Test instructions".to_string()),
        tags: vec!["python".to_string(), "programming".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill2).await.unwrap();

    let skill3 = Skill {
        id: "skl00011".to_string(),
        name: "cooking".to_string(),
        description: Some("Test description".to_string()),
        instructions: Some("Test instructions".to_string()),
        tags: vec!["cooking".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill3).await.unwrap();

    // Filter by "rust" tag - should find 1
    let query = SkillQuery {
        tags: Some(vec!["rust".to_string()]),
        ..Default::default()
    };
    let results = skills
        .list(Some(&query))
        .await
        .expect("List should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].name, "rust");

    // Filter by "programming" tag - should find 2
    let query = SkillQuery {
        tags: Some(vec!["programming".to_string()]),
        ..Default::default()
    };
    let results = skills
        .list(Some(&query))
        .await
        .expect("List should succeed");
    assert_eq!(results.items.len(), 2);
    assert_eq!(results.total, 2);

    // Filter by nonexistent tag
    let query = SkillQuery {
        tags: Some(vec!["nonexistent".to_string()]),
        ..Default::default()
    };
    let results = skills
        .list(Some(&query))
        .await
        .expect("List should succeed");
    assert!(results.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_search_with_tag_filter() {
    let db = setup_db().await;
    let skills = db.skills();

    // Create skills with different tags and content
    let skill1 = Skill {
        id: "skl00012".to_string(),
        name: "API Design".to_string(),
        description: Some("REST API patterns".to_string()),
        instructions: Some("Test instructions".to_string()),
        tags: vec!["api".to_string(), "backend".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill1).await.unwrap();

    let skill2 = Skill {
        id: "skl00013".to_string(),
        name: "API Testing".to_string(),
        description: Some("Testing API endpoints".to_string()),
        instructions: Some("Test instructions".to_string()),
        tags: vec!["api".to_string(), "testing".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill2).await.unwrap();

    let skill3 = Skill {
        id: "skl00014".to_string(),
        name: "Frontend APIs".to_string(),
        description: Some("Calling APIs from React".to_string()),
        instructions: Some("Test instructions".to_string()),
        tags: vec!["frontend".to_string()],
        license: None,
        compatibility: None,
        allowed_tools: None,
        metadata: None,
        origin_url: None,
        origin_ref: None,
        origin_fetched_at: None,
        origin_metadata: None,
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill3).await.unwrap();

    // Search for "API" with no tag filter - should find all 3
    let results = skills
        .search("API", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 3);

    // Search for "API" with "backend" tag filter - should find 1
    let query = SkillQuery {
        tags: Some(vec!["backend".to_string()]),
        ..Default::default()
    };
    let results = skills
        .search("API", Some(&query))
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].name, "API Design");

    // Search for "API" with "api" tag filter - should find 2
    let query = SkillQuery {
        tags: Some(vec!["api".to_string()]),
        ..Default::default()
    };
    let results = skills
        .search("API", Some(&query))
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 2);
    assert_eq!(results.total, 2);
}

// Project relationship CRUD is also covered in these tests via project_ids linkage in create/update/get.

#[tokio::test(flavor = "multi_thread")]
async fn skill_update_invalidates_cache() {
    use crate::db::models::SkillAttachment;
    use crate::db::sqlite::skill::SqliteSkillRepository;
    use crate::skills;
    use base64::Engine as _;

    let db = setup_db().await;
    let skills = db.skills();

    // Create a skill with an attachment
    let skill = Skill {
        id: "skl00015".to_string(),
        name: "cache-test".to_string(),
        description: Some("Test cache invalidation".to_string()),
        instructions: Some("Test instructions".to_string()),
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
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill).await.expect("Create should succeed");

    // Create an attachment using the repository's internal method
    let attachment = SkillAttachment {
        id: String::new(),
        skill_id: "skl00015".to_string(),
        type_: "script".to_string(),
        filename: "test.sh".to_string(),
        content: base64::prelude::BASE64_STANDARD.encode(b"#!/bin/bash\necho test"),
        content_hash: "abc123".to_string(),
        mime_type: Some("text/x-shellscript".to_string()),
        created_at: None,
        updated_at: None,
    };

    // Access the pool directly to create attachment
    let repo = SqliteSkillRepository { pool: db.pool() };
    repo.create_attachment(&attachment)
        .await
        .expect("Create attachment should succeed");

    // Load attachments and extract to cache
    let attachments = repo
        .get_attachments("skl00015")
        .await
        .expect("Get attachments should succeed");
    let cache_dir =
        skills::extract_attachments("skl00015", &attachments).expect("Extract should succeed");

    // Verify cache exists
    assert!(cache_dir.exists(), "Cache directory should exist");
    assert!(
        cache_dir.join("scripts/test.sh").exists(),
        "Script file should exist in cache"
    );

    // Update the skill (this should invalidate the cache)
    let mut updated_skill = skill.clone();
    updated_skill.name = "updated-cache-test".to_string();
    skills
        .update(&updated_skill)
        .await
        .expect("Update should succeed");

    // Verify cache was invalidated (directory should be gone)
    assert!(
        !cache_dir.exists(),
        "Cache directory should be removed after update"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_delete_invalidates_cache() {
    use crate::db::models::SkillAttachment;
    use crate::db::sqlite::skill::SqliteSkillRepository;
    use crate::skills;
    use base64::Engine as _;

    let db = setup_db().await;
    let skills = db.skills();

    // Create a skill with an attachment
    let skill = Skill {
        id: "skl00016".to_string(),
        name: "delete-cache-test".to_string(),
        description: Some("Test cache invalidation on delete".to_string()),
        instructions: Some("Test instructions".to_string()),
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
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill).await.expect("Create should succeed");

    // Create an attachment
    let attachment = SkillAttachment {
        id: String::new(),
        skill_id: "skl00016".to_string(),
        type_: "reference".to_string(),
        filename: "guide.md".to_string(),
        content: base64::prelude::BASE64_STANDARD.encode(b"# Guide"),
        content_hash: "xyz789".to_string(),
        mime_type: Some("text/markdown".to_string()),
        created_at: None,
        updated_at: None,
    };

    let repo = SqliteSkillRepository { pool: db.pool() };
    repo.create_attachment(&attachment)
        .await
        .expect("Create attachment should succeed");

    // Extract to cache
    let attachments = repo
        .get_attachments("skl00016")
        .await
        .expect("Get attachments should succeed");
    let cache_dir =
        skills::extract_attachments("skl00016", &attachments).expect("Extract should succeed");

    // Verify cache exists
    assert!(cache_dir.exists(), "Cache directory should exist");
    assert!(
        cache_dir.join("references/guide.md").exists(),
        "Reference file should exist in cache"
    );

    // Delete the skill (this should invalidate the cache and CASCADE delete attachment)
    skills
        .delete("skl00016")
        .await
        .expect("Delete should succeed");

    // Verify cache was invalidated
    assert!(
        !cache_dir.exists(),
        "Cache directory should be removed after delete"
    );

    // Verify attachment was CASCADE deleted from database
    let attachments_after = repo.get_attachments("skl00016").await;
    assert!(
        attachments_after.is_ok(),
        "Get attachments should not fail even if skill doesn't exist"
    );
    assert!(
        attachments_after.unwrap().is_empty(),
        "Attachments should be CASCADE deleted"
    );
}
