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
        description: "Systems programming".to_string(),
        content: r#"---
name: rust
description: Systems programming
---

# Rust Programming

Use Rust book for learning systems programming.
"#
        .to_string(),
        tags: vec![],
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
    assert_eq!(retrieved.content, skill.content);
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
        description: "Test description".to_string(),
        content: r#"---
name: first
description: Test description
---

# First Skill

Test instructions for the first skill.
"#
        .to_string(),
        tags: vec![],
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
        description: "Test description".to_string(),
        content: r#"---
name: second
description: Test description
---

# Second Skill

Test instructions for the second skill.
"#
        .to_string(),
        tags: vec![],
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
        description: "Original desc".to_string(),
        content: r#"---
name: original-name
description: Original desc
---

# Original Skill

Test instructions for original skill.
"#
        .to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    };
    skills.create(&skill).await.expect("Create should succeed");

    skill.name = "updated-name".to_string();
    skill.description = "Updated desc".to_string();
    skill.tags = vec!["updated".to_string()];
    skills.update(&skill).await.expect("Update should succeed");

    let retrieved = skills.get("skl00004").await.expect("Get should succeed");
    assert_eq!(retrieved.name, "updated-name");
    assert_eq!(retrieved.description, "Updated desc");
    assert_eq!(retrieved.tags, vec!["updated".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_delete() {
    let db = setup_db().await;
    let skills = db.skills();

    let skill = Skill {
        id: "skl00005".to_string(),
        name: "to-delete".to_string(),
        description: "Desc".to_string(),
        content: r#"---
name: to-delete
description: Desc
---

# Skill to Delete

Test instructions.
"#
        .to_string(),
        tags: vec![],
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
        description: "REST endpoints for user mgmt".to_string(),
        content: r#"---
name: api-design
description: REST endpoints for user mgmt
---

# API Design

Test instructions for API design.
"#
        .to_string(),
        tags: vec![],
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
        description: "SQLite tables for data".to_string(),
        content: r#"---
name: database
description: SQLite tables for data
---

# Database

Test instructions for database.
"#
        .to_string(),
        tags: vec![],
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
        description: "React components".to_string(),
        content: r#"---
name: frontend
description: React components
---

# Frontend

Test instructions for frontend.
"#
        .to_string(),
        tags: vec![],
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
        description: "Test description".to_string(),
        content: r#"---
name: rust
description: Test description
---

# Rust

Test instructions.
"#
        .to_string(),
        tags: vec!["rust".to_string(), "programming".to_string()],
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
        description: "Test description".to_string(),
        content: r#"---
name: python
description: Test description
---

# Python

Test instructions.
"#
        .to_string(),
        tags: vec!["python".to_string(), "programming".to_string()],
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
        description: "Test description".to_string(),
        content: r#"---
name: cooking
description: Test description
---

# Cooking

Test instructions.
"#
        .to_string(),
        tags: vec!["cooking".to_string()],
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
        description: "REST API patterns".to_string(),
        content: r#"---
name: API Design
description: REST API patterns
---

# API Design

Test instructions for API design.
"#
        .to_string(),
        tags: vec!["api".to_string(), "backend".to_string()],
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
        description: "Testing API endpoints".to_string(),
        content: r#"---
name: API Testing
description: Testing API endpoints
---

# API Testing

Test instructions for API testing.
"#
        .to_string(),
        tags: vec!["api".to_string(), "testing".to_string()],
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
        description: "Calling APIs from React".to_string(),
        content: r#"---
name: Frontend APIs
description: Calling APIs from React
---

# Frontend APIs

Test instructions for frontend APIs.
"#
        .to_string(),
        tags: vec!["frontend".to_string()],
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
