use crate::db::{
    Database, Note, NoteRepository, Project, ProjectRepository, Repo, RepoRepository, Skill,
    SkillAttachment, SkillRepository, SqliteDatabase,
};
use crate::sync::export::*;
use crate::sync::jsonl::read_jsonl;
use base64::prelude::*;
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_empty_database() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let summary = export_all(&db, temp_dir.path()).await.unwrap();

    // No default project in migrations
    assert_eq!(summary.repos, 0);
    assert_eq!(summary.projects, 0);
    assert_eq!(summary.task_lists, 0);
    assert_eq!(summary.tasks, 0);
    assert_eq!(summary.notes, 0);
    assert_eq!(summary.skills, 0);

    // Verify files exist
    assert!(temp_dir.path().join("repos.jsonl").exists());
    assert!(temp_dir.path().join("projects.jsonl").exists());
    assert!(temp_dir.path().join("lists.jsonl").exists());
    assert!(temp_dir.path().join("tasks.jsonl").exists());
    assert!(temp_dir.path().join("notes.jsonl").exists());
    assert!(temp_dir.path().join("skills.jsonl").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_with_data() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create test data
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec!["test".to_string()],
        project_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.repos().create(&repo).await.unwrap();

    let project = Project {
        id: "abcdef12".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    // Export
    let summary = export_all(&db, temp_dir.path()).await.unwrap();

    assert_eq!(summary.repos, 1);
    assert_eq!(summary.projects, 1); // Just the one we created
    assert_eq!(summary.total(), 2);

    // Verify JSONL content
    let repos: Vec<Repo> = read_jsonl(&temp_dir.path().join("repos.jsonl")).unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].id, "12345678");

    let projects: Vec<Project> = read_jsonl(&temp_dir.path().join("projects.jsonl")).unwrap();
    assert_eq!(projects.len(), 1); // Just our test project

    // Get our test project
    let our_project = &projects[0];
    assert_eq!(our_project.title, "Test Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_creates_all_files() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    export_all(&db, temp_dir.path()).await.unwrap();

    // All 6 files should exist
    let expected_files = [
        "repos.jsonl",
        "projects.jsonl",
        "lists.jsonl",
        "tasks.jsonl",
        "notes.jsonl",
        "skills.jsonl",
    ];

    for file in &expected_files {
        assert!(
            temp_dir.path().join(file).exists(),
            "File {} should exist",
            file
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_includes_relationships() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project first
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test project".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    // Create a repo linked to the project
    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec!["test".to_string()],
        project_ids: vec!["proj0001".to_string()],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.repos().create(&repo).await.unwrap();

    // Create a note with relationships
    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec!["repo0001".to_string()],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
    };
    db.notes().create(&note).await.unwrap();

    // Export
    export_all(&db, temp_dir.path()).await.unwrap();

    // Read back the exported note
    let notes: Vec<Note> = read_jsonl(&temp_dir.path().join("notes.jsonl")).unwrap();
    let exported_note = notes.iter().find(|n| n.id == "note0001").unwrap();

    // Verify relationships are exported
    assert_eq!(exported_note.repo_ids, vec!["repo0001"]);
    assert_eq!(exported_note.project_ids, vec!["proj0001"]);

    // Read back the exported project
    let projects: Vec<Project> = read_jsonl(&temp_dir.path().join("projects.jsonl")).unwrap();
    let exported_project = projects.iter().find(|p| p.id == "proj0001").unwrap();

    // Verify project relationships are exported
    assert_eq!(exported_project.note_ids, vec!["note0001"]);
    assert_eq!(exported_project.repo_ids, vec!["repo0001"]);

    // Read back the exported repo
    let repos: Vec<Repo> = read_jsonl(&temp_dir.path().join("repos.jsonl")).unwrap();
    let exported_repo = repos.iter().find(|r| r.id == "repo0001").unwrap();

    // Verify repo relationships are exported
    assert_eq!(exported_repo.project_ids, vec!["proj0001"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_skills_empty() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let summary = export_all(&db, temp_dir.path()).await.unwrap();

    // Skills should be 0 in empty database
    assert_eq!(summary.skills, 0);

    // Skills file should exist even when empty
    assert!(temp_dir.path().join("skills.jsonl").exists());

    // File should be valid JSONL (empty array when read)
    let skills: Vec<Skill> = read_jsonl(&temp_dir.path().join("skills.jsonl")).unwrap();
    assert_eq!(skills.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_skills_with_data() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project first
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test project".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    // Create a skill with project relationship
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: r#"---
name: test-skill
description: A test skill
---

# Test Skill

Do something useful.
"#
        .to_string(),
        tags: vec!["test".to_string(), "export".to_string()],
        project_ids: vec!["proj0001".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
    };
    db.skills().create(&skill).await.unwrap();

    // Export
    let summary = export_all(&db, temp_dir.path()).await.unwrap();

    // Verify summary includes skills
    assert_eq!(summary.skills, 1);
    assert_eq!(summary.total(), 2); // 1 project + 1 skill

    // Read back the exported skill
    let skills: Vec<Skill> = read_jsonl(&temp_dir.path().join("skills.jsonl")).unwrap();
    assert_eq!(skills.len(), 1);

    let exported_skill = &skills[0];
    assert_eq!(exported_skill.id, "skill001");
    assert_eq!(exported_skill.name, "test-skill");
    assert_eq!(exported_skill.description, "A test skill");
    assert!(exported_skill.content.contains("Do something"));
    assert_eq!(exported_skill.tags, vec!["test", "export"]);
    assert_eq!(exported_skill.project_ids, vec!["proj0001"]);
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
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
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
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
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
    };
    db.skills().create(&skill_no_projects).await.unwrap();

    // Export
    export_all(&db, temp_dir.path()).await.unwrap();

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
    };
    db.skills().create(&skill).await.unwrap();

    // Export
    let summary = export_all(&db, temp_dir.path()).await.unwrap();
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

#[tokio::test(flavor = "multi_thread")]
async fn test_export_skills_with_attachments() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a skill with attachments
    let skill = Skill {
        id: "skill001".to_string(),
        name: "deploy-k8s".to_string(),
        description: "Deploy to Kubernetes".to_string(),
        content: r#"---
name: deploy-k8s
description: Deploy to Kubernetes
---

# Deployment

Run scripts/deploy.sh
"#
        .to_string(),
        tags: vec!["kubernetes".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
    };
    db.skills().create(&skill).await.unwrap();

    // Create attachments for the skill
    let script_attachment = SkillAttachment {
        id: "att00001".to_string(),
        skill_id: "skill001".to_string(),
        type_: "script".to_string(),
        filename: "deploy.sh".to_string(),
        content: base64::prelude::BASE64_STANDARD.encode(b"#!/bin/bash\necho 'deploying'"),
        content_hash: "abc123".to_string(),
        mime_type: Some("text/x-shellscript".to_string()),
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
    };
    db.skills()
        .create_attachment(&script_attachment)
        .await
        .unwrap();

    let reference_attachment = SkillAttachment {
        id: "att00002".to_string(),
        skill_id: "skill001".to_string(),
        type_: "reference".to_string(),
        filename: "api-docs.md".to_string(),
        content: base64::prelude::BASE64_STANDARD.encode(b"# API Documentation"),
        content_hash: "def456".to_string(),
        mime_type: Some("text/markdown".to_string()),
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
    };
    db.skills()
        .create_attachment(&reference_attachment)
        .await
        .unwrap();

    // Export
    let summary = export_all(&db, temp_dir.path()).await.unwrap();
    assert_eq!(summary.skills, 1);

    // Read back the exported skill with attachments
    let skill_exports: Vec<SkillExport> =
        read_jsonl(&temp_dir.path().join("skills.jsonl")).unwrap();
    assert_eq!(skill_exports.len(), 1);

    let exported = &skill_exports[0];

    // Verify skill fields
    assert_eq!(exported.skill.id, "skill001");
    assert_eq!(exported.skill.name, "deploy-k8s");
    assert_eq!(exported.skill.description, "Deploy to Kubernetes");
    assert!(exported.skill.content.contains("Run scripts/deploy.sh"));

    // Verify attachments are included
    assert_eq!(exported.attachments.len(), 2);

    // Verify script attachment
    let script = exported
        .attachments
        .iter()
        .find(|a| a.type_ == "script")
        .unwrap();
    assert_eq!(script.filename, "deploy.sh");
    assert_eq!(script.content_hash, "abc123");
    assert_eq!(script.mime_type, Some("text/x-shellscript".to_string()));
    // Verify base64 content can be decoded
    let decoded = base64::prelude::BASE64_STANDARD
        .decode(&script.content)
        .unwrap();
    assert_eq!(decoded, b"#!/bin/bash\necho 'deploying'");

    // Verify reference attachment
    let reference = exported
        .attachments
        .iter()
        .find(|a| a.type_ == "reference")
        .unwrap();
    assert_eq!(reference.filename, "api-docs.md");
    assert_eq!(reference.content_hash, "def456");
    assert_eq!(reference.mime_type, Some("text/markdown".to_string()));
    let decoded = base64::prelude::BASE64_STANDARD
        .decode(&reference.content)
        .unwrap();
    assert_eq!(decoded, b"# API Documentation");
}
