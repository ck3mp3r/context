//! Tests for SyncRepository import behavior.

use crate::SqliteDatabase;
use context_core::{
    Database, HasProjects, HasRepos, HasSync, HasTaskLists, Project, ProjectRepository, Repo,
    RepoRepository, SyncRepository, TaskListRepository, TaskListStatus, write_jsonl,
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
