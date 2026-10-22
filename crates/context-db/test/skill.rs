//! Tests for SqliteSkillRepository.

use crate::SqliteDatabase;
use crate::helpers::{count_rows, read_deleted_at, read_updated_at};
use context_core::{Database, HasSkills, Skill, SkillAttachment, SkillQuery, SkillRepository};

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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
    };
    skills.create(&skill).await.expect("Create should succeed");

    skills
        .delete("skl00005")
        .await
        .expect("Delete should succeed");

    let deleted_at = read_deleted_at(db.pool(), "skill", "skl00005").await;
    assert!(deleted_at.is_some(), "skill should be soft-deleted");

    let updated_at = read_updated_at(db.pool(), "skill", "skl00005").await;
    assert_ne!(
        updated_at,
        Some("2025-01-01 00:00:00".to_string()),
        "updated_at should be bumped by the soft-delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_delete_cascades_to_relationships_and_attachments() {
    let db = setup_db().await;
    let skills = db.skills();

    sqlx::query(
        "INSERT INTO project (id, title, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("projskll")
    .bind("Skill Project")
    .bind("[]")
    .bind("2025-01-01 00:00:00")
    .bind("2025-01-01 00:00:00")
    .execute(db.pool())
    .await
    .unwrap();

    let skill = Skill {
        id: "skl00006".to_string(),
        name: "cascade-skill".to_string(),
        description: "Desc".to_string(),
        content: "---\nname: cascade-skill\ndescription: Desc\n---\n\n# Cascade Skill\n"
            .to_string(),
        tags: vec![],
        project_ids: vec!["projskll".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };
    skills.create(&skill).await.expect("Create should succeed");

    let attachment = SkillAttachment {
        id: "att00001".to_string(),
        skill_id: "skl00006".to_string(),
        type_: "script".to_string(),
        filename: "deploy.sh".to_string(),
        content: "ZWNobyBoaQ==".to_string(),
        content_hash: "hash0001".to_string(),
        mime_type: Some("text/x-shellscript".to_string()),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    skills
        .create_attachment(&attachment)
        .await
        .expect("Create attachment should succeed");

    assert_eq!(
        count_rows(db.pool(), "project_skill", "skill_id = ?", "skl00006").await,
        1,
        "project_skill row should exist before delete"
    );

    skills
        .delete("skl00006")
        .await
        .expect("Delete should succeed");

    assert!(
        read_deleted_at(db.pool(), "skill", "skl00006")
            .await
            .is_some(),
        "skill should be soft-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "project_skill", "skill_id = ?", "skl00006").await,
        0,
        "project_skill rows should be hard-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "skill_attachment", "skill_id = ?", "skl00006").await,
        0,
        "skill_attachment rows should be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_already_deleted_is_noop() {
    let db = setup_db().await;
    let skills = db.skills();

    let skill = Skill {
        id: "skl00007".to_string(),
        name: "noop-skill".to_string(),
        description: "Desc".to_string(),
        content: "---\nname: noop-skill\ndescription: Desc\n---\n\n# Noop Skill\n".to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };
    skills.create(&skill).await.expect("Create should succeed");

    skills.delete("skl00007").await.unwrap();
    let first_deleted_at = read_deleted_at(db.pool(), "skill", "skl00007").await;
    let first_updated_at = read_updated_at(db.pool(), "skill", "skl00007").await;

    skills
        .delete("skl00007")
        .await
        .expect("Second delete should be a no-op");

    assert_eq!(
        read_deleted_at(db.pool(), "skill", "skl00007").await,
        first_deleted_at,
        "deleted_at should not change on a repeated delete"
    );
    assert_eq!(
        read_updated_at(db.pool(), "skill", "skl00007").await,
        first_updated_at,
        "updated_at should not be bumped on a repeated delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_nonexistent_skill_returns_not_found() {
    let db = setup_db().await;

    let result = db.skills().delete("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "deleting a missing skill should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_attachment_soft_deletes_and_is_idempotent() {
    let db = setup_db().await;
    let skills = db.skills();

    let skill = Skill {
        id: "skl00008".to_string(),
        name: "attachment-skill".to_string(),
        description: "Desc".to_string(),
        content: "---\nname: attachment-skill\ndescription: Desc\n---\n\n# Attachment Skill\n"
            .to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };
    skills.create(&skill).await.expect("Create should succeed");

    let attachment = SkillAttachment {
        id: "att00002".to_string(),
        skill_id: "skl00008".to_string(),
        type_: "reference".to_string(),
        filename: "notes.md".to_string(),
        content: "IyBOb3Rlcw==".to_string(),
        content_hash: "hash0002".to_string(),
        mime_type: Some("text/markdown".to_string()),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    skills.create_attachment(&attachment).await.unwrap();

    skills.delete_attachment("att00002").await.unwrap();
    let first_deleted_at = read_deleted_at(db.pool(), "skill_attachment", "att00002").await;
    let first_updated_at = read_updated_at(db.pool(), "skill_attachment", "att00002").await;

    assert!(
        first_deleted_at.is_some(),
        "attachment should be soft-deleted"
    );

    skills
        .delete_attachment("att00002")
        .await
        .expect("Second delete should be a no-op");

    assert_eq!(
        read_deleted_at(db.pool(), "skill_attachment", "att00002").await,
        first_deleted_at,
        "deleted_at should not change on a repeated delete"
    );
    assert_eq!(
        read_updated_at(db.pool(), "skill_attachment", "att00002").await,
        first_updated_at,
        "updated_at should not be bumped on a repeated delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_nonexistent_attachment_returns_not_found() {
    let db = setup_db().await;

    let result = db.skills().delete_attachment("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "deleting a missing attachment should return NotFound, got: {:?}",
        result
    );
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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
        deleted_at: None,
        source: None,
        source_subpath: None,
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

// =============================================================================
// Transaction Atomicity Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_skill_with_invalid_project_should_rollback() {
    let db = setup_db().await;
    let skills = db.skills();

    // Try to create a skill with an invalid project_id (foreign key violation)
    let skill = Skill {
        id: "test1234".to_string(),
        name: "Test Skill".to_string(),
        description: "Test description".to_string(),
        content: r#"---
name: test
---
# Test"#
            .to_string(),
        tags: vec![],
        project_ids: vec!["nonexistent_project".to_string()], // Invalid project_id
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };

    let skill_id = skill.id.clone();
    let result = skills.create(&skill).await;

    // Should fail due to foreign key constraint
    assert!(
        result.is_err(),
        "Create should fail with invalid project_id: {:?}",
        result
    );

    // Verify the skill was NOT created (transaction rolled back)
    let get_result = skills.get(&skill_id).await;
    assert!(
        get_result.is_err(),
        "Skill should not exist after failed transaction"
    );
}

fn make_skill(id: &str, name: &str) -> Skill {
    Skill {
        id: id.to_string(),
        name: name.to_string(),
        description: "Test skill".to_string(),
        content: format!(
            "---\nname: {}\ndescription: Test skill\n---\n\n# Test\n",
            name
        ),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_soft_deleted_returns_not_found() {
    let db = setup_db().await;
    let skills = db.skills();

    skills
        .create(&make_skill("skldel02", "deleted-skill"))
        .await
        .expect("Create should succeed");
    skills
        .delete("skldel02")
        .await
        .expect("Delete should succeed");

    let result = skills.get("skldel02").await;
    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "get on a soft-deleted skill should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_excludes_soft_deleted() {
    let db = setup_db().await;
    let skills = db.skills();

    skills
        .create(&make_skill("skllive1", "live-skill"))
        .await
        .expect("Create should succeed");
    skills
        .create(&make_skill("skldead1", "dead-skill"))
        .await
        .expect("Create should succeed");
    skills
        .delete("skldead1")
        .await
        .expect("Delete should succeed");

    let result = skills.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1, "list should exclude tombstones");
    assert_eq!(result.items[0].id, "skllive1");
    assert_eq!(result.total, 1, "list total should exclude tombstones");
    assert_eq!(
        skills.count().await.expect("Count should succeed"),
        1,
        "count should exclude tombstones"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn count_deleted_counts_tombstones_only() {
    let db = setup_db().await;
    let skills = db.skills();

    skills
        .create(&make_skill("cntlive1", "cnt-live-skill"))
        .await
        .expect("Create should succeed");
    skills
        .create(&make_skill("cntdead1", "cnt-dead-skill"))
        .await
        .expect("Create should succeed");

    assert_eq!(
        skills
            .count_deleted()
            .await
            .expect("Count deleted should succeed"),
        0,
        "count_deleted should be zero without tombstones"
    );

    skills
        .delete("cntdead1")
        .await
        .expect("Delete should succeed");

    assert_eq!(
        skills
            .count_deleted()
            .await
            .expect("Count deleted should succeed"),
        1,
        "count_deleted should count the tombstone"
    );
    assert_eq!(
        skills.count().await.expect("Count should succeed"),
        1,
        "count should exclude the tombstone"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn count_deleted_attachments_counts_tombstones_only() {
    let db = setup_db().await;
    let skills = db.skills();

    skills
        .create(&make_skill("sklcnt01", "attachment-count-skill"))
        .await
        .expect("Create skill should succeed");

    for (id, filename) in [("attcnt01", "live.sh"), ("attcnt02", "dead.sh")] {
        skills
            .create_attachment(&SkillAttachment {
                id: id.to_string(),
                skill_id: "sklcnt01".to_string(),
                type_: "script".to_string(),
                filename: filename.to_string(),
                content: "ZWNobyBoaQ==".to_string(),
                content_hash: format!("hash-{id}"),
                mime_type: Some("text/x-shellscript".to_string()),
                created_at: Some("2025-01-01 00:00:00".to_string()),
                updated_at: Some("2025-01-01 00:00:00".to_string()),
                deleted_at: None,
            })
            .await
            .expect("Create attachment should succeed");
    }

    assert_eq!(
        skills
            .count_deleted_attachments()
            .await
            .expect("Count deleted attachments should succeed"),
        0,
        "count_deleted_attachments should be zero without tombstones"
    );

    skills
        .delete_attachment("attcnt02")
        .await
        .expect("Delete attachment should succeed");

    assert_eq!(
        skills
            .count_deleted_attachments()
            .await
            .expect("Count deleted attachments should succeed"),
        1,
        "count_deleted_attachments should count the tombstone"
    );
    assert_eq!(
        skills
            .count_attachments()
            .await
            .expect("Count attachments should succeed"),
        1,
        "count_attachments should exclude the tombstone"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_attachments_excludes_soft_deleted() {
    let db = setup_db().await;
    let skills = db.skills();

    skills
        .create(&make_skill("sklatt01", "attachment-skill"))
        .await
        .expect("Create should succeed");

    let attachment = SkillAttachment {
        id: "attdel01".to_string(),
        skill_id: "sklatt01".to_string(),
        type_: "script".to_string(),
        filename: "run.sh".to_string(),
        content: "IyEvYmluL2Jhc2g=".to_string(),
        content_hash: "hashdel".to_string(),
        mime_type: Some("text/x-shellscript".to_string()),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    skills
        .create_attachment(&attachment)
        .await
        .expect("Create attachment should succeed");

    let before = skills.get("sklatt01").await.expect("Get should succeed");
    assert_eq!(before.scripts, vec!["run.sh".to_string()]);

    skills
        .delete_attachment("attdel01")
        .await
        .expect("Delete attachment should succeed");

    let after = skills.get("sklatt01").await.expect("Get should succeed");
    assert!(
        after.scripts.is_empty(),
        "load_attachments must exclude soft-deleted attachments, got: {:?}",
        after.scripts
    );

    let attachments = skills
        .get_attachments("sklatt01")
        .await
        .expect("Get attachments should succeed");
    assert!(
        attachments.is_empty(),
        "get_attachments must exclude soft-deleted attachments, got: {:?}",
        attachments
    );
}

// =============================================================================
// Tombstone purge tests
// =============================================================================

fn make_purge_skill(id: &str) -> Skill {
    Skill {
        id: id.to_string(),
        name: format!("skill-{}", id),
        description: "Purge test skill".to_string(),
        content: format!(
            "---\nname: skill-{}\ndescription: Purge test skill\n---\n",
            id
        ),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_removes_old_tombstones() {
    let db = setup_db().await;
    let skills = db.skills();

    skills.create(&make_purge_skill("sklprg01")).await.unwrap();
    skills
        .create_attachment(&SkillAttachment {
            id: "attprg01".to_string(),
            skill_id: "sklprg01".to_string(),
            type_: "script".to_string(),
            filename: "purge.sh".to_string(),
            content: "ZWNobyBoaQ==".to_string(),
            content_hash: "hashprg1".to_string(),
            mime_type: Some("text/x-shellscript".to_string()),
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
            deleted_at: None,
        })
        .await
        .unwrap();

    skills.delete("sklprg01").await.unwrap();
    sqlx::query("UPDATE skill SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("sklprg01")
        .execute(db.pool())
        .await
        .unwrap();

    skills
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "skill", "id = ?", "sklprg01").await,
        0,
        "an old skill tombstone must be hard-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "skill_attachment", "skill_id = ?", "sklprg01").await,
        0,
        "attachments of a purged skill must be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_preserves_recent_tombstones() {
    let db = setup_db().await;
    let skills = db.skills();

    skills.create(&make_purge_skill("sklprg02")).await.unwrap();
    skills.delete("sklprg02").await.unwrap();

    skills
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "skill", "id = ?", "sklprg02").await,
        1,
        "a tombstone inside the retention period must be kept"
    );
}

// ========== Import source tests ==========

#[tokio::test(flavor = "multi_thread")]
async fn skill_table_has_nullable_source_columns() {
    use sqlx::Row;

    let db = setup_db().await;

    let rows = sqlx::query("PRAGMA table_info(skill)")
        .fetch_all(db.pool())
        .await
        .expect("table info should be readable");

    let mut found = Vec::new();
    for row in &rows {
        let name: String = row.get("name");
        if name == "source" || name == "source_subpath" {
            let not_null: i64 = row.get("notnull");
            assert_eq!(not_null, 0, "{name} must be nullable");
            found.push(name);
        }
    }

    assert_eq!(
        found.len(),
        2,
        "skill table must have source and source_subpath columns"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_create_and_get_preserves_source() {
    let db = setup_db().await;
    let skills = db.skills();

    let mut skill = make_skill("sklsrc01", "sourced-skill");
    skill.source = Some("git+https://github.com/test/sourced".to_string());
    skill.source_subpath = Some("skills/sourced".to_string());

    skills.create(&skill).await.expect("Create should succeed");

    let retrieved = skills.get("sklsrc01").await.expect("Get should succeed");
    assert_eq!(
        retrieved.source,
        Some("git+https://github.com/test/sourced".to_string()),
        "create must persist source"
    );
    assert_eq!(
        retrieved.source_subpath,
        Some("skills/sourced".to_string()),
        "create must persist source_subpath"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_create_without_source_stores_null() {
    let db = setup_db().await;
    let skills = db.skills();

    skills
        .create(&make_skill("sklsrc02", "unsourced-skill"))
        .await
        .expect("Create should succeed");

    let retrieved = skills.get("sklsrc02").await.expect("Get should succeed");
    assert_eq!(retrieved.source, None, "an absent source must read as NULL");
    assert_eq!(retrieved.source_subpath, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_update_replaces_source() {
    let db = setup_db().await;
    let skills = db.skills();

    let mut skill = make_skill("sklsrc03", "updated-source-skill");
    skill.source = Some("git+https://github.com/test/old".to_string());
    skill.source_subpath = Some("skills/old".to_string());
    skills.create(&skill).await.expect("Create should succeed");

    let mut updated = skill.clone();
    updated.source = Some("git+https://github.com/test/new".to_string());
    updated.source_subpath = Some("skills/new".to_string());
    skills
        .update(&updated)
        .await
        .expect("Update should succeed");

    let retrieved = skills.get("sklsrc03").await.expect("Get should succeed");
    assert_eq!(
        retrieved.source,
        Some("git+https://github.com/test/new".to_string()),
        "update must replace source"
    );
    assert_eq!(
        retrieved.source_subpath,
        Some("skills/new".to_string()),
        "update must replace source_subpath"
    );
}
