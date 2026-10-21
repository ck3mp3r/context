//! Tests for SyncRepository export of skills.

use crate::SqliteDatabase;
use context_core::{
    Database, HasProjects, HasSkills, HasSync, Project, ProjectRepository, Skill, SkillRepository,
    SyncRepository, read_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_skills_preserves_relationships() {
    let db = setup_test_db().await;
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
    db.projects().create(&project1).await.unwrap();

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
    db.projects().create(&project2).await.unwrap();

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

Test instructions.
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
    };
    db.skills().create(&skill).await.unwrap();

    // Create skill with no projects
    let skill_no_projects = Skill {
        id: "skill002".to_string(),
        name: "standalone-skill".to_string(),
        description: "Test description".to_string(),
        content: r#"---
name: standalone-skill
description: Test description
---

# Standalone Skill

Test instructions.
"#
        .to_string(),
        tags: vec![],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db.skills().create(&skill_no_projects).await.unwrap();

    // Export
    db.sync().export_all(temp_dir.path()).await.unwrap();

    // Read back the exported skills
    let skills: Vec<Skill> = read_jsonl(&temp_dir.path().join("skills.jsonl")).unwrap();
    assert_eq!(skills.len(), 2);

    // Verify multi-project skill
    let multi_project = skills.iter().find(|s| s.id == "skill001").unwrap();
    assert_eq!(multi_project.project_ids.len(), 2);
    assert!(multi_project.project_ids.contains(&"proj0001".to_string()));
    assert!(multi_project.project_ids.contains(&"proj0002".to_string()));

    // Verify standalone skill
    let standalone = skills.iter().find(|s| s.id == "skill002").unwrap();
    assert_eq!(standalone.project_ids.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_skills_with_agent_skills_fields() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a skill with ALL Agent Skills standard fields populated
    let skill = Skill {
        id: "skill001".to_string(),
        name: "deploy-kubernetes".to_string(),
        description: "Deploy applications to Kubernetes cluster with validation".to_string(),
        content: r#"---
name: deploy-kubernetes
description: Deploy applications to Kubernetes cluster with validation
license: Apache-2.0
compatibility: Requires kubectl, docker
allowed-tools: ["Bash(kubectl:*)","Bash(docker:*)"]
metadata:
  author: ck3mp3r
  version: "1.0"
origin:
  url: https://github.com/user/repo
  ref: main
  fetched_at: "2026-01-31T10:00:00Z"
  metadata:
    commit: abc123
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
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db.skills().create(&skill).await.unwrap();

    // Export
    let summary = db.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(summary.skills, 1);

    // Read back the exported skill
    let skills: Vec<Skill> = read_jsonl(&temp_dir.path().join("skills.jsonl")).unwrap();
    assert_eq!(skills.len(), 1);

    let exported = &skills[0];

    // Verify core fields
    assert_eq!(exported.id, "skill001");
    assert_eq!(exported.name, "deploy-kubernetes");
    assert_eq!(
        exported.description,
        "Deploy applications to Kubernetes cluster with validation"
    );
    assert!(exported.content.contains("# Deployment Steps"));
    assert!(exported.content.contains("Run validation"));
    assert_eq!(exported.tags, vec!["kubernetes", "deployment"]);

    // Verify Agent Skills standard fields are in content
    assert!(exported.content.contains("license: Apache-2.0"));
    assert!(
        exported
            .content
            .contains("compatibility: Requires kubectl, docker")
    );
    assert!(
        exported
            .content
            .contains(r#"["Bash(kubectl:*)","Bash(docker:*)"]"#)
    );
    assert!(exported.content.contains("author: ck3mp3r"));

    // Verify origin tracking fields are in content (these are now in frontmatter, not separate DB fields)
    assert!(
        exported
            .content
            .contains("url: https://github.com/user/repo")
    );
    assert!(exported.content.contains("ref: main"));
    assert!(
        exported
            .content
            .contains(r#"fetched_at: "2026-01-31T10:00:00Z""#)
    );
}
