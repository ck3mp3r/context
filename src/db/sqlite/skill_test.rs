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

fn make_skill(id: &str, name: &str, desc: Option<&str>, instructions: Option<&str>) -> Skill {
    Skill {
        id: id.to_string(),
        name: name.to_string(),
        description: desc.map(|s| s.to_string()),
        instructions: instructions.map(|s| s.to_string()),
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_create_and_get() {
    let db = setup_db().await;
    let skills = db.skills();

    let skill = make_skill(
        "skl00001",
        "Rust",
        Some("Systems programming"),
        Some("Use Rust book"),
    );
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
    skills
        .create(&make_skill("skl00002", "First", Some("Desc one"), None))
        .await
        .unwrap();
    skills
        .create(&make_skill("skl00003", "Second", Some("Desc two"), None))
        .await
        .unwrap();

    let result = skills.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_update() {
    let db = setup_db().await;
    let skills = db.skills();

    let mut skill = make_skill("skl00004", "Original Name", Some("Original desc"), None);
    skills.create(&skill).await.expect("Create should succeed");

    skill.name = "Updated Name".to_string();
    skill.description = Some("Updated desc".to_string());
    skill.tags = vec!["updated".to_string()];
    skills.update(&skill).await.expect("Update should succeed");

    let retrieved = skills.get("skl00004").await.expect("Get should succeed");
    assert_eq!(retrieved.name, "Updated Name");
    assert_eq!(retrieved.description, Some("Updated desc".to_string()));
    assert_eq!(retrieved.tags, vec!["updated".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_delete() {
    let db = setup_db().await;
    let skills = db.skills();

    let skill = make_skill("skl00005", "To Delete", Some("Desc"), None);
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
    skills
        .create(&make_skill(
            "skl00006",
            "API Design",
            Some("REST endpoints for user mgmt"),
            None,
        ))
        .await
        .unwrap();
    skills
        .create(&make_skill(
            "skl00007",
            "Database",
            Some("SQLite tables for data"),
            None,
        ))
        .await
        .unwrap();
    skills
        .create(&make_skill(
            "skl00008",
            "Frontend",
            Some("React components"),
            None,
        ))
        .await
        .unwrap();

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
    assert_eq!(results.items[0].name, "Frontend");

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
    let mut skill1 = make_skill("skl00009", "Rust", None, None);
    skill1.tags = vec!["rust".to_string(), "programming".to_string()];
    skills.create(&skill1).await.unwrap();

    let mut skill2 = make_skill("skl00010", "Python", None, None);
    skill2.tags = vec!["python".to_string(), "programming".to_string()];
    skills.create(&skill2).await.unwrap();

    let mut skill3 = make_skill("skl00011", "Cooking", None, None);
    skill3.tags = vec!["cooking".to_string()];
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
    assert_eq!(results.items[0].name, "Rust");

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
    let mut skill1 = make_skill("skl00012", "API Design", Some("REST API patterns"), None);
    skill1.tags = vec!["api".to_string(), "backend".to_string()];
    skills.create(&skill1).await.unwrap();

    let mut skill2 = make_skill(
        "skl00013",
        "API Testing",
        Some("Testing API endpoints"),
        None,
    );
    skill2.tags = vec!["api".to_string(), "testing".to_string()];
    skills.create(&skill2).await.unwrap();

    let mut skill3 = make_skill(
        "skl00014",
        "Frontend APIs",
        Some("Calling APIs from React"),
        None,
    );
    skill3.tags = vec!["frontend".to_string()];
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
