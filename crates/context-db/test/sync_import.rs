//! Tests for SyncRepository import behavior.

use crate::SqliteDatabase;
use context_core::{
    Database, HasProjects, HasRepos, HasSync, HasTaskLists, Note, Project, ProjectRepository, Repo,
    RepoRepository, Skill, SkillAttachment, SyncRepository, Task, TaskListRepository,
    TaskListStatus, TaskStatus, write_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

/// Helper to create test JSONL files in different orders.
fn create_test_jsonl_repos_before_projects(temp_dir: &TempDir) {
    // Create a repo that references a project
    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec![],
        project_ids: vec!["proj0001".to_string()], // FK reference
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };

    // Create the project being referenced
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test project".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };

    // Write repos FIRST (before projects exist) - this would normally fail FK
    write_jsonl(&temp_dir.path().join("repos.jsonl"), &[repo]).unwrap();
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();

    // Create empty files for other entities
    write_jsonl::<context_core::TaskList>(&temp_dir.path().join("lists.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Task>(&temp_dir.path().join("tasks.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();
}

/// Helper to create JSONL with invalid FK reference.
fn create_invalid_fk_jsonl(temp_dir: &TempDir) {
    // Create a repo referencing a non-existent project
    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec![],
        // Invalid FK. The ID is 8 characters so the length CHECK
        // constraint passes and the deferred FK check is what fails.
        project_ids: vec!["deadbeef".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };

    write_jsonl(&temp_dir.path().join("repos.jsonl"), &[repo]).unwrap();
    write_jsonl::<Project>(&temp_dir.path().join("projects.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::TaskList>(&temp_dir.path().join("lists.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Task>(&temp_dir.path().join("tasks.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();
}

/// Helper to create JSONL with partial valid data and one invalid FK.
fn create_partial_invalid_jsonl(temp_dir: &TempDir) {
    // Valid project
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

    // Valid repo
    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec!["proj0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };

    // Invalid task_list with bad project FK
    let task_list = context_core::TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: None,
        project_id: "badproject".to_string(), // Invalid FK
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        notes: None,
        repo_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };

    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();
    write_jsonl(&temp_dir.path().join("repos.jsonl"), &[repo]).unwrap();
    write_jsonl(&temp_dir.path().join("lists.jsonl"), &[task_list]).unwrap();
    write_jsonl::<context_core::Task>(&temp_dir.path().join("tasks.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_database_has_sync_repository() {
    // RED: Database trait doesn't have sync() method yet
    let db = setup_test_db().await;

    // This should compile when Database trait has sync() method
    let _sync_repo = db.sync();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sync_repository_import_all_interface() {
    // RED: SyncRepository trait doesn't exist
    let db = setup_test_db().await;

    let temp_dir = TempDir::new().unwrap();

    // This should compile when SyncRepository trait exists
    let result = db.sync().import_all(temp_dir.path()).await;

    // Should return ImportSummary on success
    assert!(result.is_ok());
}

// ========== Phase 3: FK Deferred Tests ==========

#[tokio::test(flavor = "multi_thread")]
async fn test_import_repos_before_projects_succeeds() {
    // RED: Will fail because import doesn't defer FK yet
    let db = setup_test_db().await;

    let temp_dir = TempDir::new().unwrap();

    // Create JSONL files with repos referencing projects
    // BUT repos.jsonl is read BEFORE projects.jsonl in import logic
    create_test_jsonl_repos_before_projects(&temp_dir);

    // Should succeed due to deferred FK
    let result = db.sync().import_all(temp_dir.path()).await;

    assert!(
        result.is_ok(),
        "Import should succeed with deferred FK, but got error: {:?}",
        result.err()
    );

    let summary = result.unwrap();
    assert_eq!(summary.repos, 1, "Should import 1 repo");
    assert_eq!(summary.projects, 1, "Should import 1 project");

    // Verify data actually inserted correctly
    let repos = db.repos().list(None).await.unwrap();
    assert_eq!(repos.items.len(), 1, "Should have 1 repo in database");

    let repo = db.repos().get("repo0001").await.unwrap();
    assert_eq!(
        repo.project_ids,
        vec!["proj0001"],
        "Repo should reference project"
    );

    let projects = db.projects().list(None).await.unwrap();
    // Note: migrations create 1 default project, so we expect 2 total
    assert!(
        !projects.items.is_empty(),
        "Should have at least 1 project in database"
    );

    let project = db.projects().get("proj0001").await.unwrap();
    assert_eq!(project.title, "Test Project");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_with_invalid_fk_fails_at_commit() {
    // RED: Will fail because we don't validate FK at commit yet
    let db = setup_test_db().await;

    let temp_dir = TempDir::new().unwrap();

    // Create repo referencing non-existent project
    create_invalid_fk_jsonl(&temp_dir);

    let result = db.sync().import_all(temp_dir.path()).await;

    // Should fail with FK constraint error
    assert!(
        result.is_err(),
        "Import should fail with FK constraint violation"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string().to_lowercase();
    assert!(
        err_msg.contains("foreign key")
            || err_msg.contains("constraint")
            || err_msg.contains("foreign_key"),
        "Error should mention FK constraint, got: {}",
        err
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_rolls_back_on_error() {
    // RED: Will fail because we don't rollback properly yet
    let db = setup_test_db().await;

    // Insert a valid project first
    let project = Project {
        id: "proj0001".to_string(),
        title: "Original Project".to_string(),
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

    let temp_dir = TempDir::new().unwrap();

    // Create JSONL with:
    // 1. Valid project (should upsert proj0001)
    // 2. Valid repo
    // 3. Invalid task_list (bad FK to non-existent project)
    create_partial_invalid_jsonl(&temp_dir);

    let result = db.sync().import_all(temp_dir.path()).await;

    // Should fail due to invalid task_list FK
    assert!(
        result.is_err(),
        "Import should fail due to invalid task_list FK"
    );

    // Verify NOTHING was imported (rollback occurred)
    let repos = db.repos().list(None).await.unwrap();
    assert_eq!(repos.items.len(), 0, "Rollback should prevent repo insert");

    // Original project should still exist unchanged
    let project = db.projects().get("proj0001").await.unwrap();
    assert_eq!(
        project.title, "Original Project",
        "Original project should be unchanged after rollback"
    );

    // Should have no task lists
    let lists = db.task_lists().list(None).await.unwrap();
    assert_eq!(
        lists.items.len(),
        0,
        "Rollback should prevent task_list insert"
    );
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

    let summary = db.sync().import_all(temp_dir.path()).await.unwrap();

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
    let summary = db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(summary.total(), 0);
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
    let summary = db.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(summary.skills, 0);
}

// ========== GATE 1: empty database is a full import ==========

/// Count every row in `table`, including soft-deleted rows.
async fn count_table_rows(pool: &sqlx::SqlitePool, table: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {}", table);
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .fetch_one(pool)
        .await
        .expect("count should succeed")
}

/// Write a full replica with `count` rows in each of the seven entity tables.
fn write_full_replica(temp_dir: &TempDir, count: usize) {
    let created_at = Some("2024-01-01 00:00:00".to_string());
    let updated_at = Some("2024-06-01 00:00:00".to_string());

    let projects: Vec<Project> = (0..count)
        .map(|i| Project {
            id: format!("projfl{i:02}"),
            title: format!("Project {i}"),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
        })
        .collect();

    let repos: Vec<Repo> = (0..count)
        .map(|i| Repo {
            id: format!("repofl{i:02}"),
            remote: format!("github:test/full{i}"),
            path: None,
            tags: vec![],
            project_ids: vec![format!("projfl{i:02}")],
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
        })
        .collect();

    let task_lists: Vec<context_core::TaskList> = (0..count)
        .map(|i| context_core::TaskList {
            id: format!("listfl{i:02}"),
            title: format!("List {i}"),
            description: None,
            notes: None,
            project_id: format!("projfl{i:02}"),
            tags: vec![],
            status: TaskListStatus::Active,
            external_refs: vec![],
            repo_ids: vec![format!("repofl{i:02}")],
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            archived_at: None,
            deleted_at: None,
        })
        .collect();

    let tasks: Vec<Task> = (0..count)
        .map(|i| Task {
            id: format!("taskfl{i:02}"),
            list_id: format!("listfl{i:02}"),
            parent_id: None,
            title: format!("Task {i}"),
            description: None,
            status: TaskStatus::Todo,
            priority: Some(3),
            tags: vec![],
            external_refs: vec![],
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
        })
        .collect();

    let notes: Vec<Note> = (0..count)
        .map(|i| Note {
            id: format!("notefl{i:02}"),
            title: format!("Note {i}"),
            content: format!("Content {i}"),
            tags: vec![],
            parent_id: None,
            idx: None,
            repo_ids: vec![format!("repofl{i:02}")],
            project_ids: vec![format!("projfl{i:02}")],
            subnote_count: None,
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
        })
        .collect();

    let skills: Vec<Skill> = (0..count)
        .map(|i| Skill {
            id: format!("skilfl{i:02}"),
            name: format!("full-skill-{i}"),
            description: format!("Full skill {i}"),
            content: format!(
                "---\nname: full-skill-{i}\ndescription: Full skill {i}\n---\n\n# Full Skill {i}\n"
            ),
            tags: vec![],
            project_ids: vec![format!("projfl{i:02}")],
            scripts: vec![],
            references: vec![],
            assets: vec![],
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
        })
        .collect();

    let attachments: Vec<SkillAttachment> = (0..count)
        .map(|i| SkillAttachment {
            id: format!("attafl{i:02}"),
            skill_id: format!("skilfl{i:02}"),
            type_: "script".to_string(),
            filename: format!("script-{i}.sh"),
            content: "ZWNobyBoaQo=".to_string(),
            content_hash: format!("hash-{i}"),
            mime_type: Some("text/x-shellscript".to_string()),
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
        })
        .collect();

    write_jsonl(&temp_dir.path().join("projects.jsonl"), &projects).unwrap();
    write_jsonl(&temp_dir.path().join("repos.jsonl"), &repos).unwrap();
    write_jsonl(&temp_dir.path().join("lists.jsonl"), &task_lists).unwrap();
    write_jsonl(&temp_dir.path().join("tasks.jsonl"), &tasks).unwrap();
    write_jsonl(&temp_dir.path().join("notes.jsonl"), &notes).unwrap();
    write_jsonl(&temp_dir.path().join("skills.jsonl"), &skills).unwrap();
    write_jsonl(
        &temp_dir.path().join("skills_attachments.jsonl"),
        &attachments,
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_empty_database_writes_every_entity_row() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    let rows_per_table = 3;

    write_full_replica(&temp_dir, rows_per_table);

    let summary = db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(summary.projects, rows_per_table);
    assert_eq!(summary.repos, rows_per_table);
    assert_eq!(summary.task_lists, rows_per_table);
    assert_eq!(summary.tasks, rows_per_table);
    assert_eq!(summary.notes, rows_per_table);
    assert_eq!(summary.skills, rows_per_table);
    assert_eq!(summary.attachments, rows_per_table);

    // No target rows exist, so the guard pre-filter must keep every row.
    let tables = [
        "project",
        "repo",
        "task_list",
        "task",
        "note",
        "skill",
        "skill_attachment",
    ];
    for table in tables {
        assert_eq!(
            count_table_rows(db.pool(), table).await,
            rows_per_table as i64,
            "{table} must hold every row from its file"
        );
    }
}
