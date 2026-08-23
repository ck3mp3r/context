//! Integration tests for skill import functionality.
//!
//! These tests were moved from context-skills (Phase 5) and use the
//! test harness in common::setup_db() to construct the database.

mod common;

use context_core::{HasProjects, Project, ProjectRepository, generate_entity_id};
use context_db::SqliteDatabase;
use context_skills::import_skill;

async fn setup_db() -> SqliteDatabase {
    common::setup_db().await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_local_path() {
    let db = setup_db().await;

    // Create a temporary skill directory
    let temp_dir = std::env::temp_dir().join(format!("test-skill-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create SKILL.md
    let skill_md = temp_dir.join("SKILL.md");
    std::fs::write(
        &skill_md,
        r#"---
name: Test Skill
description: A test skill
---

# Test Skill

This is a test skill for import testing.
"#,
    )
    .unwrap();

    // Create a script
    std::fs::create_dir_all(temp_dir.join("scripts")).unwrap();
    std::fs::write(temp_dir.join("scripts/test.sh"), "#!/bin/bash\necho test").unwrap();

    // Import the skill (no upsert)
    let result = import_skill(&db, temp_dir.to_str().unwrap(), None, None, None, false).await;

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();

    assert!(result.is_ok(), "Import should succeed");
    let skill = result.unwrap();
    assert_eq!(skill.name, "Test Skill");
    assert_eq!(skill.description, "A test skill");
    assert_eq!(skill.scripts.len(), 1);
    assert_eq!(skill.scripts[0], "scripts/test.sh"); // Full relative path
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_duplicate_without_update_fails() {
    let db = setup_db().await;

    // Create a temporary skill directory with unique ID to avoid conflicts
    let temp_dir = std::env::temp_dir().join(format!("test-skill-{}", generate_entity_id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create SKILL.md
    let skill_md = temp_dir.join("SKILL.md");
    std::fs::write(
        &skill_md,
        r#"---
name: Test Skill
description: A test skill
---

# Test Skill
"#,
    )
    .unwrap();

    // First import should succeed
    let result1 = import_skill(&db, temp_dir.to_str().unwrap(), None, None, None, false).await;
    assert!(result1.is_ok(), "First import should succeed");

    // Second import without update flag should fail
    let result2 = import_skill(&db, temp_dir.to_str().unwrap(), None, None, None, false).await;
    assert!(result2.is_err(), "Second import without update should fail");

    match result2.unwrap_err() {
        context_skills::ImportError::ValidationError(msg) => {
            assert!(msg.contains("already exists"));
            assert!(msg.contains("--update"));
        }
        _ => panic!("Expected ValidationError"),
    }

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_duplicate_with_update_succeeds() {
    let db = setup_db().await;

    // Create a temporary skill directory with unique ID to avoid conflicts
    let temp_dir = std::env::temp_dir().join(format!("test-skill-update-{}", generate_entity_id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create SKILL.md
    let skill_md = temp_dir.join("SKILL.md");
    std::fs::write(
        &skill_md,
        r#"---
name: Test Update Skill
description: Original description
---

# Test Skill
"#,
    )
    .unwrap();

    // First import
    let result1 = import_skill(&db, temp_dir.to_str().unwrap(), None, None, None, false).await;
    assert!(result1.is_ok(), "First import should succeed");
    let skill1 = result1.unwrap();
    assert_eq!(skill1.description, "Original description");

    // Update SKILL.md
    std::fs::write(
        &skill_md,
        r#"---
name: Test Update Skill
description: Updated description
---

# Test Skill Updated
"#,
    )
    .unwrap();

    // Second import with update flag should succeed
    let result2 = import_skill(&db, temp_dir.to_str().unwrap(), None, None, None, true).await;
    assert!(result2.is_ok(), "Second import with update should succeed");
    let skill2 = result2.unwrap();
    assert_eq!(skill2.id, skill1.id, "ID should be the same");
    assert_eq!(skill2.description, "Updated description");

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_update_preserves_tags_and_project_ids_when_not_provided() {
    let db = setup_db().await;

    // Create a test project for FK constraint
    let project = Project {
        id: generate_entity_id(),
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
    db.projects()
        .create(&project)
        .await
        .expect("Project creation should succeed");

    let temp_dir =
        std::env::temp_dir().join(format!("test-skill-preserve-{}", generate_entity_id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let skill_md = temp_dir.join("SKILL.md");
    std::fs::write(
        &skill_md,
        r#"---
name: Preserve Test Skill
description: Test preservation
---

# Test
"#,
    )
    .unwrap();

    // First import WITH tags and project_ids
    let initial_tags = Some(vec!["tag1".to_string(), "tag2".to_string()]);
    let initial_projects = Some(vec![project.id.clone()]);
    let result1 = import_skill(
        &db,
        temp_dir.to_str().unwrap(),
        None,
        initial_projects.clone(),
        initial_tags.clone(),
        false,
    )
    .await;
    assert!(
        result1.is_ok(),
        "First import should succeed: {:?}",
        result1.err()
    );
    let skill1 = result1.unwrap();
    assert_eq!(skill1.tags, vec!["tag1", "tag2"]);
    assert_eq!(skill1.project_ids, vec![project.id.clone()]);

    // Update SKILL.md content
    std::fs::write(
        &skill_md,
        r#"---
name: Preserve Test Skill
description: Updated content
---

# Updated
"#,
    )
    .unwrap();

    // Re-import with update=true but WITHOUT tags/project_ids
    // Expected: should preserve existing tags and project_ids
    let result2 = import_skill(&db, temp_dir.to_str().unwrap(), None, None, None, true).await;
    assert!(result2.is_ok(), "Update import should succeed");
    let skill2 = result2.unwrap();
    assert_eq!(skill2.id, skill1.id, "ID should be the same");
    assert_eq!(skill2.description, "Updated content");
    // THIS IS THE KEY ASSERTION - tags/project_ids should be preserved
    assert_eq!(
        skill2.tags,
        vec!["tag1", "tag2"],
        "Tags should be preserved when not provided"
    );
    assert_eq!(
        skill2.project_ids,
        vec![project.id.clone()],
        "Project IDs should be preserved when not provided"
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_update_replaces_tags_and_project_ids_when_provided() {
    let db = setup_db().await;

    // Create two test projects
    let old_project = Project {
        id: generate_entity_id(),
        title: "Old Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.projects()
        .create(&old_project)
        .await
        .expect("Old project creation should succeed");

    let new_project = Project {
        id: generate_entity_id(),
        title: "New Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.projects()
        .create(&new_project)
        .await
        .expect("New project creation should succeed");

    let temp_dir =
        std::env::temp_dir().join(format!("test-skill-replace-{}", generate_entity_id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let skill_md = temp_dir.join("SKILL.md");
    std::fs::write(
        &skill_md,
        r#"---
name: Replace Test Skill
description: Test replacement
---

# Test
"#,
    )
    .unwrap();

    // First import WITH tags and project_ids
    let initial_tags = Some(vec!["old-tag".to_string()]);
    let initial_projects = Some(vec![old_project.id.clone()]);
    let result1 = import_skill(
        &db,
        temp_dir.to_str().unwrap(),
        None,
        initial_projects,
        initial_tags,
        false,
    )
    .await;
    assert!(result1.is_ok(), "First import should succeed");
    let skill1 = result1.unwrap();
    assert_eq!(skill1.tags, vec!["old-tag"]);
    assert_eq!(skill1.project_ids, vec![old_project.id.clone()]);

    // Re-import with update=true and DIFFERENT tags/project_ids
    // Expected: should REPLACE with new values
    let new_tags = Some(vec!["new-tag".to_string()]);
    let new_projects = Some(vec![new_project.id.clone()]);
    let result2 = import_skill(
        &db,
        temp_dir.to_str().unwrap(),
        None,
        new_projects,
        new_tags,
        true,
    )
    .await;
    assert!(result2.is_ok(), "Update import should succeed");
    let skill2 = result2.unwrap();
    assert_eq!(skill2.id, skill1.id, "ID should be the same");
    assert_eq!(
        skill2.tags,
        vec!["new-tag"],
        "Tags should be replaced when provided"
    );
    assert_eq!(
        skill2.project_ids,
        vec![new_project.id.clone()],
        "Project IDs should be replaced when provided"
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_update_adds_tags_and_project_ids_to_empty_skill() {
    let db = setup_db().await;

    // Create a test project
    let project = Project {
        id: generate_entity_id(),
        title: "Added Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: None,
        updated_at: None,
    };
    db.projects()
        .create(&project)
        .await
        .expect("Project creation should succeed");

    let temp_dir = std::env::temp_dir().join(format!("test-skill-add-{}", generate_entity_id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let skill_md = temp_dir.join("SKILL.md");
    std::fs::write(
        &skill_md,
        r#"---
name: Add Test Skill
description: Test adding
---

# Test
"#,
    )
    .unwrap();

    // First import WITHOUT tags and project_ids
    let result1 = import_skill(&db, temp_dir.to_str().unwrap(), None, None, None, false).await;
    assert!(result1.is_ok(), "First import should succeed");
    let skill1 = result1.unwrap();
    assert!(skill1.tags.is_empty());
    assert!(skill1.project_ids.is_empty());

    // Re-import with update=true and tags/project_ids
    // Expected: should ADD the new values
    let new_tags = Some(vec!["added-tag".to_string()]);
    let new_projects = Some(vec![project.id.clone()]);
    let result2 = import_skill(
        &db,
        temp_dir.to_str().unwrap(),
        None,
        new_projects,
        new_tags,
        true,
    )
    .await;
    assert!(result2.is_ok(), "Update import should succeed");
    let skill2 = result2.unwrap();
    assert_eq!(skill2.id, skill1.id, "ID should be the same");
    assert_eq!(
        skill2.tags,
        vec!["added-tag"],
        "Tags should be added when provided"
    );
    assert_eq!(
        skill2.project_ids,
        vec![project.id.clone()],
        "Project IDs should be added when provided"
    );

    std::fs::remove_dir_all(&temp_dir).ok();
}
