//! Tests for SyncRepository import of skill Agent Skills fields.

use crate::SqliteDatabase;
use context_core::{Database, HasSkills, HasSync, Skill, SkillRepository, SyncRepository};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_export_skills_agent_skills_fields_round_trip() {
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a skill in db1 with ALL Agent Skills fields populated
    let skill = Skill {
        id: "skill001".to_string(),
        name: "deploy-kubernetes".to_string(),
        description: "Deploy applications to Kubernetes cluster with validation".to_string(),
        content: r#"---
name: deploy-kubernetes
description: Deploy applications to Kubernetes cluster with validation
license: Apache-2.0
compatibility: Requires kubectl, docker
allowed-tools: ["Bash(kubectl:*)", "Bash(docker:*)"]
metadata:
  author: ck3mp3r
  version: "1.0"
  category: deployment
origin:
  url: https://github.com/user/repo
  ref: main
  fetched_at: 2026-01-31T10:00:00Z
  metadata:
    commit: abc123
    branch: main
---

# Deployment Steps

1. Run validation
2. Apply manifests
"#
        .to_string(),
        tags: vec!["kubernetes".to_string(), "deployment".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T12:00:00Z".to_string()),
        updated_at: Some("2024-01-01T15:30:00Z".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };
    db1.skills().create(&skill).await.unwrap();

    // Export from db1
    let export_summary = db1.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.skills, 1);

    // Import to db2
    let import_summary = db2.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(import_summary.skills, 1);

    // Verify ALL fields survived round-trip
    let imported = db2.skills().get("skill001").await.unwrap();

    // Core fields
    assert_eq!(imported.id, "skill001");
    assert_eq!(imported.name, "deploy-kubernetes");
    assert_eq!(
        imported.description,
        "Deploy applications to Kubernetes cluster with validation"
    );
    assert!(imported.content.contains("# Deployment Steps"));
    assert!(imported.content.contains("Run validation"));
    assert!(imported.content.contains("Apply manifests"));
    assert_eq!(imported.tags, vec!["kubernetes", "deployment"]);

    // Agent Skills standard fields (now in content frontmatter)
    assert!(imported.content.contains("license: Apache-2.0"));
    assert!(
        imported
            .content
            .contains("compatibility: Requires kubectl, docker")
    );
    assert!(imported.content.contains(r#"["Bash(kubectl:*)""#));
    assert!(imported.content.contains("author: ck3mp3r"));
    assert!(imported.content.contains(r#"version: "1.0""#));
    assert!(imported.content.contains("category: deployment"));

    // Origin tracking fields (now in content frontmatter)
    assert!(
        imported
            .content
            .contains("url: https://github.com/user/repo")
    );
    assert!(imported.content.contains("ref: main"));
    assert!(
        imported
            .content
            .contains("fetched_at: 2026-01-31T10:00:00Z")
    );
    assert!(imported.content.contains("commit: abc123"));
    assert!(imported.content.contains("branch: main"));

    // Timestamps
    assert_eq!(
        imported.created_at,
        Some("2024-01-01T12:00:00Z".to_string())
    );
    assert_eq!(
        imported.updated_at,
        Some("2024-01-01T15:30:00Z".to_string())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_skills_upsert_updates_agent_skills_fields() {
    // Tests the UPSERT code path - updating an existing skill with new Agent Skills metadata
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create initial skill with basic fields
    let initial_skill = Skill {
        id: "skill002".to_string(),
        name: "initial-name".to_string(),
        description: "Initial description".to_string(),
        content: r#"---
name: initial-name
description: Initial description
license: MIT
compatibility: opencode>=0.1.0
allowed-tools: ["Bash(echo:*)"]
metadata:
  version: "1.0"
origin:
  url: https://github.com/original/repo
  ref: main
---

# Initial Skill

Initial instructions
"#
        .to_string(),
        tags: vec!["tag1".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T10:00:00Z".to_string()),
        updated_at: Some("2024-01-01T10:00:00Z".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };
    db.skills().create(&initial_skill).await.unwrap();

    // Export (to create JSONL file)
    db.sync().export_all(temp_dir.path()).await.unwrap();

    // Modify the JSONL to simulate external changes to Agent Skills fields
    let skills_file = temp_dir.path().join("skills.jsonl");
    let modified_skill = Skill {
        id: "skill002".to_string(),
        name: "updated-name".to_string(),
        description: "Updated description with new content".to_string(),
        content: r#"---
name: updated-name
description: Updated description with new content
license: Apache-2.0
compatibility: Requires kubectl >= 1.30, docker >= 20.10
allowed-tools: ["Bash(kubectl:*)", "Bash(docker:*)", "Bash(helm:*)"]
metadata:
  version: "2.0"
  author: updated
  new_field: added
origin:
  url: https://github.com/updated/repo
  ref: v2.0.0
  fetched_at: 2026-01-31T19:00:00Z
  metadata:
    commit: xyz789
    updated: true
---

# Updated Skill

Updated instructions with changes
"#
        .to_string(),
        tags: vec![
            "tag1".to_string(),
            "tag2".to_string(),
            "updated".to_string(),
        ],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T10:00:00Z".to_string()),
        updated_at: Some("2026-01-31T19:00:00Z".to_string()), // CHANGED
        deleted_at: None,
        source: None,
        source_subpath: None,
    };
    std::fs::write(
        &skills_file,
        format!("{}\n", serde_json::to_string(&modified_skill).unwrap()),
    )
    .unwrap();

    // Import - this should UPSERT (update existing skill)
    let import_summary = db.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(import_summary.skills, 1);

    // Verify ALL Agent Skills fields were updated via UPSERT
    let updated = db.skills().get("skill002").await.unwrap();

    // Core fields should be updated
    assert_eq!(updated.name, "updated-name");
    assert_eq!(updated.description, "Updated description with new content");
    assert!(
        updated
            .content
            .contains("Updated instructions with changes")
    );
    assert_eq!(updated.tags, vec!["tag1", "tag2", "updated"]);

    // **CRITICAL**: Agent Skills fields should be updated via UPSERT (now in content frontmatter)
    assert!(updated.content.contains("license: Apache-2.0")); // Must be updated
    assert!(
        updated
            .content
            .contains("compatibility: Requires kubectl >= 1.30, docker >= 20.10")
    ); // Must be updated
    assert!(updated.content.contains(r#"["Bash(kubectl:*)""#)); // Must be updated
    assert!(updated.content.contains(r#"version: "2.0""#)); // Must be updated
    assert!(updated.content.contains("author: updated")); // Must be updated
    assert!(updated.content.contains("new_field: added")); // Must be updated

    // **CRITICAL**: Origin tracking fields should be updated via UPSERT (now in content frontmatter)
    assert!(
        updated
            .content
            .contains("url: https://github.com/updated/repo")
    ); // Must be updated
    assert!(updated.content.contains("ref: v2.0.0")); // Must be updated
    assert!(updated.content.contains("fetched_at: 2026-01-31T19:00:00Z")); // Must be updated
    assert!(updated.content.contains("commit: xyz789")); // Must be updated
    assert!(updated.content.contains("updated: true")); // Must be updated

    // Timestamps
    assert_eq!(updated.created_at, Some("2024-01-01T10:00:00Z".to_string())); // Should not change
    assert_eq!(updated.updated_at, Some("2026-01-31T19:00:00Z".to_string())); // Should be updated
}
