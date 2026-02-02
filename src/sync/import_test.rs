use crate::db::{
    Database, Note, NoteRepository, Project, ProjectRepository, Repo, RepoRepository, Skill,
    SkillAttachment, SkillRepository, SqliteDatabase, TaskList, TaskListRepository, TaskListStatus,
};
use crate::sync::export::{SkillExport, export_all};
use crate::sync::import::*;
use crate::sync::jsonl::write_jsonl;
use base64::prelude::*;
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_empty_directory() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create empty JSONL files
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("projects.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("skills.jsonl"), "").unwrap();

    let summary = import_all(&db, temp_dir.path()).await.unwrap();

    assert_eq!(summary.repos, 0);
    assert_eq!(summary.projects, 0);
    assert_eq!(summary.task_lists, 0);
    assert_eq!(summary.tasks, 0);
    assert_eq!(summary.notes, 0);
    assert_eq!(summary.skills, 0);
    assert_eq!(summary.total(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_missing_files() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Don't create any files - should handle gracefully
    let summary = import_all(&db, temp_dir.path()).await.unwrap();

    assert_eq!(summary.total(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_then_import() {
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create test data in db1
    let repo = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec!["test".to_string()],
        project_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db1.repos().create(&repo).await.unwrap();

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
    db1.projects().create(&project).await.unwrap();

    // Export from db1
    let export_summary = export_all(&db1, temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.repos, 1);
    assert_eq!(export_summary.projects, 1); // Just test project

    // Import to db2
    let import_summary = import_all(&db2, temp_dir.path()).await.unwrap();
    assert_eq!(import_summary.repos, 1);
    assert_eq!(import_summary.projects, 1); // Just test project

    // Verify data in db2
    let repos = db2.repos().list(None).await.unwrap();
    assert_eq!(repos.items.len(), 1);
    assert_eq!(repos.items[0].id, "12345678");

    let projects = db2.projects().list(None).await.unwrap();
    // db2 has just the imported project
    assert_eq!(projects.items.len(), 1);

    // Get our imported project
    let imported_project = &projects.items[0];
    assert_eq!(imported_project.title, "Test Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_updates_existing() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create initial repo
    let repo_v1 = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo1".to_string(),
        path: Some("/test/path1".to_string()),
        tags: vec!["v1".to_string()],
        project_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.repos().create(&repo_v1).await.unwrap();

    // Create modified version in JSONL
    let repo_v2 = Repo {
        id: "12345678".to_string(),
        remote: "https://github.com/test/repo2".to_string(), // Changed
        path: Some("/test/path2".to_string()),               // Changed
        tags: vec!["v2".to_string()],                        // Changed
        project_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };

    write_jsonl(&temp_dir.path().join("repos.jsonl"), &[repo_v2]).unwrap();

    // Create empty files for other entities
    std::fs::write(temp_dir.path().join("projects.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    // Import should update the existing repo
    let summary = import_all(&db, temp_dir.path()).await.unwrap();
    assert_eq!(summary.repos, 1);

    // Verify it was updated, not duplicated
    let repos = db.repos().list(None).await.unwrap();
    assert_eq!(repos.items.len(), 1);

    let updated_repo = db.repos().get("12345678").await.unwrap();
    assert_eq!(updated_repo.remote, "https://github.com/test/repo2");
    assert_eq!(updated_repo.path, Some("/test/path2".to_string()));
    assert_eq!(updated_repo.tags, vec!["v2".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_preserves_relationships() {
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create entities with relationships in db1
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db1.projects().create(&project).await.unwrap();

    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec!["proj0001".to_string()],
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db1.repos().create(&repo).await.unwrap();

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
    db1.notes().create(&note).await.unwrap();

    // Export from db1
    export_all(&db1, temp_dir.path()).await.unwrap();

    // Import to db2
    import_all(&db2, temp_dir.path()).await.unwrap();

    // Verify relationships are preserved
    let imported_project = db2.projects().get("proj0001").await.unwrap();
    assert_eq!(imported_project.repo_ids, vec!["repo0001"]);
    assert_eq!(imported_project.note_ids, vec!["note0001"]);

    let imported_repo = db2.repos().get("repo0001").await.unwrap();
    assert_eq!(imported_repo.project_ids, vec!["proj0001"]);

    let imported_note = db2.notes().get("note0001").await.unwrap();
    assert_eq!(imported_note.project_ids, vec!["proj0001"]);
    assert_eq!(imported_note.repo_ids, vec!["repo0001"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_preserves_timestamps() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a note with specific timestamps
    let original_created = "2024-01-01T10:00:00Z";
    let original_updated = "2024-01-02T15:30:00Z";

    let note = Note {
        id: "testid01".to_string(),
        title: "Test Note".to_string(),
        content: "Original content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some(original_created.to_string()),
        updated_at: Some(original_updated.to_string()),
    };

    db.notes().create(&note).await.unwrap();

    // Verify timestamps were stored correctly
    let stored_note = db.notes().get("testid01").await.unwrap();
    assert_eq!(stored_note.created_at, Some(original_created.to_string()));
    assert_eq!(stored_note.updated_at, Some(original_updated.to_string()));

    // Export the note
    export_all(&db, temp_dir.path()).await.unwrap();

    // Modify the note content and update timestamp
    let modified_updated = "2024-01-03T20:45:00Z";
    let modified_note = Note {
        id: "testid01".to_string(),
        title: "Test Note".to_string(),
        content: "Modified content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        created_at: Some(original_created.to_string()),
        updated_at: Some(modified_updated.to_string()),
    };

    // Write the modified note to JSONL (simulating external modification)
    write_jsonl(&temp_dir.path().join("notes.jsonl"), &[modified_note]).unwrap();

    // Import should preserve the modified timestamp
    import_all(&db, temp_dir.path()).await.unwrap();

    // Verify timestamps were preserved (not overwritten by trigger)
    let imported_note = db.notes().get("testid01").await.unwrap();
    assert_eq!(
        imported_note.created_at,
        Some(original_created.to_string()),
        "created_at should be preserved"
    );
    assert_eq!(
        imported_note.updated_at,
        Some(modified_updated.to_string()),
        "updated_at should be preserved from import, not auto-generated"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_preserves_project_timestamps() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project with specific timestamps
    let original_created = "2024-01-01T10:00:00Z";
    let original_updated = "2024-01-02T15:30:00Z";

    let project = Project {
        id: "proj1234".to_string(),
        title: "Test Project".to_string(),
        description: Some("Test description".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: original_created.to_string(),
        updated_at: original_updated.to_string(),
    };

    db.projects().create(&project).await.unwrap();

    // Modify and reimport with new timestamp
    let modified_updated = "2024-01-03T20:45:00Z";
    let modified_project = Project {
        id: "proj1234".to_string(),
        title: "Modified Project".to_string(),
        description: Some("Modified description".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: original_created.to_string(),
        updated_at: modified_updated.to_string(),
    };

    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[modified_project]).unwrap();
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    import_all(&db, temp_dir.path()).await.unwrap();

    let imported_project = db.projects().get("proj1234").await.unwrap();
    assert_eq!(
        imported_project.created_at, original_created,
        "project created_at should be preserved"
    );
    assert_eq!(
        imported_project.updated_at, modified_updated,
        "project updated_at should be preserved from import, not auto-generated"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_preserves_task_list_timestamps() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project first (required for task list)
    let project = Project {
        id: "proj1234".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db.projects().create(&project).await.unwrap();

    // Create a task list with specific timestamps
    let original_created = "2024-01-01T10:00:00Z";
    let original_updated = "2024-01-02T15:30:00Z";

    let task_list = TaskList {
        id: "list1234".to_string(),
        title: "Test List".to_string(),
        description: Some("Test description".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: "proj1234".to_string(),
        created_at: original_created.to_string(),
        updated_at: original_updated.to_string(),
        archived_at: None,
    };

    db.task_lists().create(&task_list).await.unwrap();

    // Modify and reimport with new timestamp
    let modified_updated = "2024-01-03T20:45:00Z";
    let modified_list = TaskList {
        id: "list1234".to_string(),
        title: "Modified List".to_string(),
        description: Some("Modified description".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        notes: None,
        repo_ids: vec![],
        project_id: "proj1234".to_string(),
        created_at: original_created.to_string(),
        updated_at: modified_updated.to_string(),
        archived_at: None,
    };

    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();
    write_jsonl(&temp_dir.path().join("lists.jsonl"), &[modified_list]).unwrap();
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    import_all(&db, temp_dir.path()).await.unwrap();

    let imported_list = db.task_lists().get("list1234").await.unwrap();
    assert_eq!(
        imported_list.created_at, original_created,
        "task_list created_at should be preserved"
    );
    assert_eq!(
        imported_list.updated_at, modified_updated,
        "task_list updated_at should be preserved from import, not auto-generated"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_skills_creates_new() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project first
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    // Create a skill
    let skill = Skill {
        id: "skill001".to_string(),
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        content: r#"---
name: test-skill
description: A test skill
---

# Test Skill

Do something
"#
        .to_string(),
        tags: vec!["test".to_string()],
        project_ids: vec!["proj0001".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
    };

    // Write to JSONL
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();
    write_jsonl(&temp_dir.path().join("skills.jsonl"), &[skill]).unwrap();
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    // Import
    let summary = import_all(&db, temp_dir.path()).await.unwrap();
    assert_eq!(summary.skills, 1);

    // Verify skill was created
    let imported_skill = db.skills().get("skill001").await.unwrap();
    assert_eq!(imported_skill.name, "test-skill");
    assert_eq!(imported_skill.description, "A test skill");
    assert!(imported_skill.content.contains("Do something"));
    assert_eq!(imported_skill.tags, vec!["test"]);
    assert_eq!(imported_skill.project_ids, vec!["proj0001"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_skills_updates_existing() {
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
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
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

Original instructions
"#
        .to_string(),
        tags: vec!["v1".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T10:00:00Z".to_string()),
    };
    db.skills().create(&skill_v1).await.unwrap();

    // Create modified version for import
    let skill_v2 = Skill {
        id: "skill001".to_string(),
        name: "updated-name".to_string(),
        description: "Updated description".to_string(),
        content: r#"---
name: updated-name
description: Updated description
---

# Updated Skill

Updated instructions
"#
        .to_string(),
        tags: vec!["v2".to_string()],
        project_ids: vec!["proj0001".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-02T15:00:00Z".to_string()),
    };

    // Write to JSONL
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();
    write_jsonl(&temp_dir.path().join("skills.jsonl"), &[skill_v2]).unwrap();
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    // Import should update
    let summary = import_all(&db, temp_dir.path()).await.unwrap();
    assert_eq!(summary.skills, 1);

    // Verify only one skill exists (updated, not duplicated)
    let skills = db.skills().list(None).await.unwrap();
    assert_eq!(skills.items.len(), 1);

    // Verify it was updated
    let updated_skill = db.skills().get("skill001").await.unwrap();
    assert_eq!(updated_skill.name, "updated-name");
    assert_eq!(updated_skill.description, "Updated description");
    assert!(updated_skill.content.contains("Updated instructions"));
    assert_eq!(updated_skill.tags, vec!["v2"]);
    assert_eq!(updated_skill.project_ids, vec!["proj0001"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_skills_preserves_project_relationships() {
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

Test instructions
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

    // Write to JSONL
    write_jsonl(
        &temp_dir.path().join("projects.jsonl"),
        &[project1, project2],
    )
    .unwrap();
    write_jsonl(&temp_dir.path().join("skills.jsonl"), &[skill]).unwrap();
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    // Import
    import_all(&db, temp_dir.path()).await.unwrap();

    // Verify M:N relationships preserved
    let imported_skill = db.skills().get("skill001").await.unwrap();
    assert_eq!(imported_skill.project_ids.len(), 2);
    assert!(imported_skill.project_ids.contains(&"proj0001".to_string()));
    assert!(imported_skill.project_ids.contains(&"proj0002".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_missing_skills_file_handled_gracefully() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create other files but skip skills.jsonl
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("projects.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();
    // No skills.jsonl

    // Import should succeed with 0 skills
    let summary = import_all(&db, temp_dir.path()).await.unwrap();
    assert_eq!(summary.skills, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_skills_round_trip() {
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create data in db1
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    db1.projects().create(&project).await.unwrap();

    let skill = Skill {
        id: "skill001".to_string(),
        name: "round-trip-skill".to_string(),
        description: "Testing round-trip".to_string(),
        content: r#"---
name: round-trip-skill
description: Testing round-trip
---

# Round Trip Skill

Should survive export/import
"#
        .to_string(),
        tags: vec!["test".to_string(), "round-trip".to_string()],
        project_ids: vec!["proj0001".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T12:00:00Z".to_string()),
        updated_at: Some("2024-01-01T15:30:00Z".to_string()),
    };
    db1.skills().create(&skill).await.unwrap();

    // Export from db1
    let export_summary = export_all(&db1, temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.skills, 1);

    // Import to db2
    let import_summary = import_all(&db2, temp_dir.path()).await.unwrap();
    assert_eq!(import_summary.skills, 1);

    // Verify data integrity
    let imported_skill = db2.skills().get("skill001").await.unwrap();
    assert_eq!(imported_skill.id, "skill001");
    assert_eq!(imported_skill.name, "round-trip-skill");
    assert_eq!(imported_skill.description, "Testing round-trip");
    assert!(
        imported_skill
            .content
            .contains("Should survive export/import")
    );
    assert_eq!(imported_skill.tags, vec!["test", "round-trip"]);
    assert_eq!(imported_skill.project_ids, vec!["proj0001"]);
    assert_eq!(
        imported_skill.created_at,
        Some("2024-01-01T12:00:00Z".to_string())
    );
    assert_eq!(
        imported_skill.updated_at,
        Some("2024-01-01T15:30:00Z".to_string())
    );
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
    };
    db1.skills().create(&skill).await.unwrap();

    // Export from db1
    let export_summary = export_all(&db1, temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.skills, 1);

    // Import to db2
    let import_summary = import_all(&db2, temp_dir.path()).await.unwrap();
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
    };
    db.skills().create(&initial_skill).await.unwrap();

    // Export (to create JSONL file)
    export_all(&db, temp_dir.path()).await.unwrap();

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
    };
    std::fs::write(
        &skills_file,
        format!("{}\n", serde_json::to_string(&modified_skill).unwrap()),
    )
    .unwrap();

    // Import - this should UPSERT (update existing skill)
    let import_summary = import_all(&db, temp_dir.path()).await.unwrap();
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

#[tokio::test(flavor = "multi_thread")]
async fn test_import_skills_with_attachments_upsert() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create skill with attachments
    let skill_content = r#"---
name: test-skill
description: Test skill with attachments
version: "1.0.0"
---

# Test Skill

This skill has attachments.
"#;

    let script_content = b"#!/bin/bash\necho 'test script'";
    let script_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(script_content);
        format!("{:x}", hasher.finalize())
    };
    let script_b64 = BASE64_STANDARD.encode(script_content);

    let reference_content = b"# Reference Doc\nSome documentation";
    let reference_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(reference_content);
        format!("{:x}", hasher.finalize())
    };
    let reference_b64 = BASE64_STANDARD.encode(reference_content);

    let skill_export = SkillExport {
        skill: Skill {
            id: "sk001234".to_string(),
            name: "test-skill".to_string(),
            description: "Test skill with attachments".to_string(),
            content: skill_content.to_string(),
            tags: vec![],
            project_ids: vec![],
            scripts: vec![],
            references: vec![],
            assets: vec![],
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-01T10:00:00Z".to_string()),
        },
        attachments: vec![
            SkillAttachment {
                id: "at001234".to_string(),
                skill_id: "sk001234".to_string(),
                type_: "script".to_string(),
                filename: "run.sh".to_string(),
                content: script_b64.clone(),
                content_hash: script_hash.clone(),
                mime_type: Some("text/x-shellscript".to_string()),
                created_at: Some("2024-01-01T10:00:00Z".to_string()),
                updated_at: Some("2024-01-01T10:00:00Z".to_string()),
            },
            SkillAttachment {
                id: "at005678".to_string(),
                skill_id: "sk001234".to_string(),
                type_: "reference".to_string(),
                filename: "README.md".to_string(),
                content: reference_b64.clone(),
                content_hash: reference_hash.clone(),
                mime_type: Some("text/markdown".to_string()),
                created_at: Some("2024-01-01T10:00:00Z".to_string()),
                updated_at: Some("2024-01-01T10:00:00Z".to_string()),
            },
        ],
    };

    // Write to JSONL and import
    write_jsonl(
        &temp_dir.path().join("skills.jsonl"),
        std::slice::from_ref(&skill_export),
    )
    .unwrap();
    import_all(&db, temp_dir.path()).await.unwrap();

    // Verify skill created
    let imported_skill = db.skills().get("sk001234").await.unwrap();
    assert_eq!(imported_skill.name, "test-skill");

    // Verify attachments created
    let attachments = db.skills().get_attachments("sk001234").await.unwrap();
    assert_eq!(attachments.len(), 2);

    let script_att = attachments.iter().find(|a| a.filename == "run.sh").unwrap();
    assert_eq!(script_att.type_, "script");
    assert_eq!(script_att.content_hash, script_hash);
    assert_eq!(script_att.content, script_b64);

    let ref_att = attachments
        .iter()
        .find(|a| a.filename == "README.md")
        .unwrap();
    assert_eq!(ref_att.type_, "reference");
    assert_eq!(ref_att.content_hash, reference_hash);

    // Test 2: Re-import with changed attachment content (should update + invalidate cache)
    let modified_script = b"#!/bin/bash\necho 'modified script'";
    let modified_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(modified_script);
        format!("{:x}", hasher.finalize())
    };
    let modified_b64 = BASE64_STANDARD.encode(modified_script);

    let updated_export = SkillExport {
        skill: skill_export.skill.clone(),
        attachments: vec![
            SkillAttachment {
                id: "at001234".to_string(),
                skill_id: "sk001234".to_string(),
                type_: "script".to_string(),
                filename: "run.sh".to_string(),
                content: modified_b64.clone(),
                content_hash: modified_hash.clone(),
                mime_type: Some("text/x-shellscript".to_string()),
                created_at: Some("2024-01-01T10:00:00Z".to_string()),
                updated_at: Some("2024-01-02T10:00:00Z".to_string()),
            },
            SkillAttachment {
                id: "at005678".to_string(),
                skill_id: "sk001234".to_string(),
                type_: "reference".to_string(),
                filename: "README.md".to_string(),
                content: reference_b64.clone(),
                content_hash: reference_hash.clone(),
                mime_type: Some("text/markdown".to_string()),
                created_at: Some("2024-01-01T10:00:00Z".to_string()),
                updated_at: Some("2024-01-01T10:00:00Z".to_string()),
            },
        ],
    };

    write_jsonl(&temp_dir.path().join("skills.jsonl"), &[updated_export]).unwrap();
    import_all(&db, temp_dir.path()).await.unwrap();

    // Verify attachment updated
    let updated_attachments = db.skills().get_attachments("sk001234").await.unwrap();
    assert_eq!(updated_attachments.len(), 2);

    let updated_script = updated_attachments
        .iter()
        .find(|a| a.filename == "run.sh")
        .unwrap();
    assert_eq!(updated_script.content_hash, modified_hash);
    assert_eq!(updated_script.content, modified_b64);

    // Reference should be unchanged
    let unchanged_ref = updated_attachments
        .iter()
        .find(|a| a.filename == "README.md")
        .unwrap();
    assert_eq!(unchanged_ref.content_hash, reference_hash);

    // Test 3: Re-import with attachment removed (should delete it)
    let minimal_export = SkillExport {
        skill: skill_export.skill.clone(),
        attachments: vec![SkillAttachment {
            id: "at005678".to_string(),
            skill_id: "sk001234".to_string(),
            type_: "reference".to_string(),
            filename: "README.md".to_string(),
            content: reference_b64.clone(),
            content_hash: reference_hash.clone(),
            mime_type: Some("text/markdown".to_string()),
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-01T10:00:00Z".to_string()),
        }],
    };

    write_jsonl(&temp_dir.path().join("skills.jsonl"), &[minimal_export]).unwrap();
    import_all(&db, temp_dir.path()).await.unwrap();

    // Verify script attachment deleted
    let final_attachments = db.skills().get_attachments("sk001234").await.unwrap();
    assert_eq!(final_attachments.len(), 1);
    assert_eq!(final_attachments[0].filename, "README.md");
    assert!(final_attachments.iter().all(|a| a.filename != "run.sh"));
}
