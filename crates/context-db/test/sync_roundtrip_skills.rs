//! Tests for SyncRepository skill export/import roundtrips.

use crate::SqliteDatabase;
use base64::prelude::*;
use context_core::{
    Database, HasProjects, HasSkills, HasSync, Project, ProjectRepository, Repo, Skill,
    SkillRepository, SyncRepository, TaskList, read_jsonl, write_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_skills_with_relationships() {
    // RED: Skills not included in sync yet
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project).await.unwrap();

    // Create a skill linked to the project
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: r#"---
name: test-skill
description: A test skill
---

# Test Skill

Do something useful with this skill.
"#
        .to_string(),
        tags: vec!["test".to_string()],
        project_ids: vec!["proj0001".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
        source: Some("git+https://github.com/test/skill".to_string()),
        source_subpath: Some("skills/test-skill".to_string()),
    };
    db1.skills().create(&skill).await.unwrap();

    // Create attachments for the skill
    use context_core::SkillAttachment;
    let attachment1 = SkillAttachment {
        id: "attach01".to_string(),
        skill_id: "skill001".to_string(),
        type_: "script".to_string(),
        filename: "test.sh".to_string(),
        content: BASE64_STANDARD.encode("#!/bin/bash\necho test"),
        content_hash: "hash123".to_string(),
        mime_type: Some("text/x-shellscript".to_string()),
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.skills().create_attachment(&attachment1).await.unwrap();

    let attachment2 = SkillAttachment {
        id: "attach02".to_string(),
        skill_id: "skill001".to_string(),
        type_: "reference".to_string(),
        filename: "docs.md".to_string(),
        content: BASE64_STANDARD.encode("# Documentation"),
        content_hash: "hash456".to_string(),
        mime_type: Some("text/markdown".to_string()),
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.skills().create_attachment(&attachment2).await.unwrap();

    // Export from db1
    let export_summary = db1.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.skills, 1, "Should export 1 skill");
    assert_eq!(export_summary.attachments, 2, "Should export 2 attachments");

    // The exported attachment rows must carry each attachment's mime_type.
    let exported_attachments: Vec<SkillAttachment> =
        read_jsonl(&temp_dir.path().join("skills_attachments.jsonl")).unwrap();
    let exported_script = exported_attachments
        .iter()
        .find(|a| a.filename == "test.sh")
        .expect("skills_attachments.jsonl should contain test.sh");
    assert_eq!(
        exported_script.mime_type,
        Some("text/x-shellscript".to_string()),
        "export must write the script attachment mime_type"
    );
    let exported_reference = exported_attachments
        .iter()
        .find(|a| a.filename == "docs.md")
        .expect("skills_attachments.jsonl should contain docs.md");
    assert_eq!(
        exported_reference.mime_type,
        Some("text/markdown".to_string()),
        "export must write the reference attachment mime_type"
    );

    // Import to db2
    let import_summary = db2.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(import_summary.skills, 1, "Should import 1 skill");
    assert_eq!(import_summary.attachments, 2, "Should import 2 attachments");

    // Verify skill data and relationships
    let imported_skill = db2.skills().get("skill001").await.unwrap();
    assert_eq!(imported_skill.name, "test-skill");
    assert_eq!(imported_skill.description, "A test skill");
    assert!(imported_skill.content.contains("Do something"));
    assert_eq!(imported_skill.tags, vec!["test"]);
    assert_eq!(imported_skill.project_ids, vec!["proj0001"]);
    assert_eq!(
        imported_skill.source,
        Some("git+https://github.com/test/skill".to_string()),
        "source must survive export then import"
    );
    assert_eq!(
        imported_skill.source_subpath,
        Some("skills/test-skill".to_string()),
        "source_subpath must survive export then import"
    );

    // Verify attachments were imported
    let imported_attachments = db2.skills().get_attachments("skill001").await.unwrap();
    assert_eq!(imported_attachments.len(), 2, "Should import 2 attachments");

    let script = imported_attachments
        .iter()
        .find(|a| a.type_ == "script")
        .unwrap();
    assert_eq!(script.filename, "test.sh");
    assert_eq!(script.content_hash, "hash123");

    let reference = imported_attachments
        .iter()
        .find(|a| a.type_ == "reference")
        .unwrap();
    assert_eq!(reference.filename, "docs.md");
    assert_eq!(reference.content_hash, "hash456");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_skills_multiple_projects() {
    // RED: Skills M:N relationships not synced
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create multiple projects
    let project1 = Project {
        id: "proj0001".to_string(),
        title: "Project 1".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project1).await.unwrap();

    let project2 = Project {
        id: "proj0002".to_string(),
        title: "Project 2".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project2).await.unwrap();

    // Create skill linked to multiple projects
    let skill = Skill {
        id: "skill001".to_string(),
        name: "multi-project-skill".to_string(),
        description: "Test description".to_string(),
        content: r#"---
name: multi-project-skill
description: Test description
---

# Multi-Project Skill

Test instructions for multi-project skill.
"#
        .to_string(),
        tags: vec![],
        project_ids: vec!["proj0001".to_string(), "proj0002".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };
    db1.skills().create(&skill).await.unwrap();

    // Export and import
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify M:N relationships preserved
    let imported_skill = db2.skills().get("skill001").await.unwrap();
    assert_eq!(imported_skill.project_ids.len(), 2);
    assert!(imported_skill.project_ids.contains(&"proj0001".to_string()));
    assert!(imported_skill.project_ids.contains(&"proj0002".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_skills_upsert() {
    // RED: Skills upsert logic not implemented
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create project
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db.projects().create(&project).await.unwrap();

    // Create initial skill
    let skill_v1 = Skill {
        id: "skill001".to_string(),
        name: "original-name".to_string(),
        description: "Original description".to_string(),
        content: r#"---
name: original-name
description: Original description
---

# Original Skill

Original instructions for the skill.
"#
        .to_string(),
        tags: vec!["v1".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T10:00:00Z".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };
    db.skills().create(&skill_v1).await.unwrap();

    // Create modified version in JSONL
    let skill_v2 = Skill {
        id: "skill001".to_string(),
        name: "Updated Name".to_string(),
        description: "Updated description".to_string(),
        content: r#"---
name: Updated Name
description: Updated description
---

# Updated Skill

Updated instructions for the skill.
"#
        .to_string(),
        tags: vec!["v2".to_string()],
        project_ids: vec!["proj0001".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-02T15:00:00Z".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };

    // Write to JSONL
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();
    write_jsonl(&temp_dir.path().join("skills.jsonl"), &[skill_v2]).unwrap();
    write_jsonl::<Repo>(&temp_dir.path().join("repos.jsonl"), &[]).unwrap();
    write_jsonl::<TaskList>(&temp_dir.path().join("lists.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Task>(&temp_dir.path().join("tasks.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();

    // Import should update
    let summary = db.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(summary.skills, 1, "Should import 1 skill");

    // Verify only one skill exists (updated, not duplicated)
    let skills = db.skills().list(None).await.unwrap();
    assert_eq!(
        skills.items.len(),
        1,
        "Should have exactly 1 skill after upsert"
    );

    // Verify it was updated
    let updated_skill = db.skills().get("skill001").await.unwrap();
    assert_eq!(updated_skill.name, "Updated Name");
    assert_eq!(updated_skill.description, "Updated description");
    assert!(updated_skill.content.contains("Updated instructions"));
    assert_eq!(updated_skill.tags, vec!["v2"]);
    assert_eq!(updated_skill.project_ids, vec!["proj0001"]);
    assert_eq!(
        updated_skill.updated_at,
        Some("2024-01-02T15:00:00Z".to_string()),
        "CRITICAL: updated_at must be preserved during import!"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_skills_without_source_fields_stores_null() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // A pre-migration export has no source or source_subpath keys.
    let legacy = serde_json::json!({
        "id": "skilllg1",
        "name": "legacy-skill",
        "description": "Legacy skill",
        "content": "---\nname: legacy-skill\ndescription: Legacy skill\n---\n\n# Legacy\n",
        "tags": [],
        "project_ids": [],
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z",
        "deleted_at": null
    });
    std::fs::write(
        temp_dir.path().join("skills.jsonl"),
        format!("{}\n", legacy),
    )
    .unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let imported = db.skills().get("skilllg1").await.unwrap();
    assert_eq!(imported.name, "legacy-skill");
    assert_eq!(
        imported.source, None,
        "a missing source key must import as NULL"
    );
    assert_eq!(
        imported.source_subpath, None,
        "a missing source_subpath key must import as NULL"
    );
}
