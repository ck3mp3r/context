//! Tests for SyncRepository export/import roundtrips.

use crate::SqliteDatabase;
use base64::prelude::*;
use context_core::{
    Database, HasNotes, HasProjects, HasRepos, HasSkills, HasSync, HasTaskLists, HasTasks, Project,
    ProjectRepository, Repo, RepoRepository, Skill, SkillRepository, SyncRepository, TaskList,
    TaskListRepository, TaskListStatus, write_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_then_import_roundtrip() {
    // Create DB with data
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test project".to_string()),
        tags: vec!["test".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project).await.unwrap();

    // Create a repo linked to the project
    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec!["git".to_string()],
        project_ids: vec!["proj0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.repos().create(&repo).await.unwrap();

    // Export from db1 using sync repository
    let export_summary = db1.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.repos, 1, "Should export 1 repo");
    // Note: db1 has 1 default project + our test project = 2
    assert!(
        export_summary.projects >= 1,
        "Should export at least 1 project"
    );

    // Import to db2 using sync repository
    let import_summary = db2.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(import_summary.repos, 1, "Should import 1 repo");
    assert!(
        import_summary.projects >= 1,
        "Should import at least 1 project"
    );

    // Verify data in db2
    let imported_repo = db2.repos().get("repo0001").await.unwrap();
    assert_eq!(imported_repo.remote, "https://github.com/test/repo");
    assert_eq!(imported_repo.project_ids, vec!["proj0001"]);
    assert_eq!(imported_repo.tags, vec!["git"]);

    let imported_project = db2.projects().get("proj0001").await.unwrap();
    assert_eq!(imported_project.title, "Test Project");
    assert_eq!(imported_project.repo_ids, vec!["repo0001"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_export_preserves_all_relationships() {
    use context_core::{Note, NoteRepository, TaskList, TaskListRepository, TaskListStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create complex data with all relationship types
    let project = Project {
        id: "proj0001".to_string(),
        title: "Complex Project".to_string(),
        description: None,
        tags: vec!["complex".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project).await.unwrap();

    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec!["proj0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.repos().create(&repo).await.unwrap();

    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Task List".to_string(),
        description: None,
        notes: None,
        project_id: "proj0001".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec!["repo0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db1.task_lists().create(&task_list).await.unwrap();

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Note content".to_string(),
        tags: vec!["important".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec!["repo0001".to_string()],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&note).await.unwrap();

    // Export and import
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify all relationships preserved
    let imported_project = db2.projects().get("proj0001").await.unwrap();
    assert_eq!(imported_project.repo_ids, vec!["repo0001"]);
    assert_eq!(imported_project.task_list_ids, vec!["list0001"]);
    assert_eq!(imported_project.note_ids, vec!["note0001"]);

    let imported_repo = db2.repos().get("repo0001").await.unwrap();
    assert_eq!(imported_repo.project_ids, vec!["proj0001"]);

    let imported_list = db2.task_lists().get("list0001").await.unwrap();
    assert_eq!(imported_list.project_id, "proj0001");
    assert_eq!(imported_list.repo_ids, vec!["repo0001"]);

    let imported_note = db2.notes().get("note0001").await.unwrap();
    assert_eq!(imported_note.project_ids, vec!["proj0001"]);
    assert_eq!(imported_note.repo_ids, vec!["repo0001"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_preserves_task_updated_at() {
    use context_core::{TaskRepository, TaskStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a test project
    let project = Project {
        id: "projtest".to_string(),
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
    let project_id = project.id.clone();

    // Create a task list
    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        project_id,
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db1.task_lists().create(&task_list).await.unwrap();

    // Create task with specific timestamps directly via SQL (bypassing create() which overwrites timestamps)
    sqlx::query(
        "INSERT INTO task (id, list_id, parent_id, title, description, status, priority, tags, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind("task0001")
    .bind("list0001")
    .bind(None::<String>)
    .bind("Test Task")
    .bind(Some("Description"))
    .bind("in_progress")
    .bind(Some(2))
    .bind(r#"["test"]"#)
    .bind("2024-01-01T10:00:00Z")
    .bind("2024-01-01T12:00:00Z") // CRITICAL: Must be preserved
    .execute(db1.pool())
    .await
    .unwrap();

    // Export and import
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify updated_at is preserved
    let imported_task = db2.tasks().get("task0001").await.unwrap();
    assert_eq!(
        imported_task.updated_at,
        Some("2024-01-01T12:00:00Z".to_string()),
        "CRITICAL: updated_at must be preserved during export/import!"
    );
    assert_eq!(
        imported_task.created_at,
        Some("2024-01-01T10:00:00Z".to_string())
    );
    assert_eq!(imported_task.title, "Test Task");
    assert_eq!(imported_task.status, TaskStatus::InProgress);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_preserves_note_hierarchy() {
    // RED: This test will fail because parent_id and idx are not preserved during import
    use context_core::NoteRepository;

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

    // Create parent note
    let parent_note = context_core::Note {
        id: "note0001".to_string(),
        title: "Parent Note".to_string(),
        content: "This is the parent".to_string(),
        tags: vec!["parent".to_string()],
        parent_id: None,
        idx: Some(1),
        repo_ids: vec![],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&parent_note).await.unwrap();

    // Create child note with parent_id and idx
    let child_note = context_core::Note {
        id: "note0002".to_string(),
        title: "Child Note".to_string(),
        content: "This is a child".to_string(),
        tags: vec!["child".to_string()],
        parent_id: Some("note0001".to_string()), // CRITICAL: Must be preserved
        idx: Some(2),                            // CRITICAL: Must be preserved
        repo_ids: vec![],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-02T00:00:00Z".to_string()),
        updated_at: Some("2024-01-02T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&child_note).await.unwrap();

    // Create another child with different idx
    let child_note2 = context_core::Note {
        id: "note0003".to_string(),
        title: "Second Child".to_string(),
        content: "Another child".to_string(),
        tags: vec![],
        parent_id: Some("note0001".to_string()), // CRITICAL: Must be preserved
        idx: Some(1),                            // CRITICAL: Different idx for ordering
        repo_ids: vec![],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-03T00:00:00Z".to_string()),
        updated_at: Some("2024-01-03T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&child_note2).await.unwrap();

    // Export and import
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify parent_id and idx are preserved
    let imported_parent = db2.notes().get("note0001").await.unwrap();
    assert_eq!(
        imported_parent.parent_id, None,
        "Parent should have no parent_id"
    );
    assert_eq!(
        imported_parent.idx,
        Some(1),
        "Parent idx should be preserved"
    );

    let imported_child = db2.notes().get("note0002").await.unwrap();
    assert_eq!(
        imported_child.parent_id,
        Some("note0001".to_string()),
        "CRITICAL: child parent_id must be preserved during export/import!"
    );
    assert_eq!(
        imported_child.idx,
        Some(2),
        "CRITICAL: child idx must be preserved during export/import!"
    );

    let imported_child2 = db2.notes().get("note0003").await.unwrap();
    assert_eq!(
        imported_child2.parent_id,
        Some("note0001".to_string()),
        "CRITICAL: second child parent_id must be preserved!"
    );
    assert_eq!(
        imported_child2.idx,
        Some(1),
        "CRITICAL: second child idx must be preserved!"
    );
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
