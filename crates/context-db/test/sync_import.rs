//! Tests for SyncRepository import behavior.

use crate::SqliteDatabase;
use base64::prelude::*;
use context_core::{
    Database, HasNotes, HasProjects, HasRepos, HasSkills, HasSync, HasTaskLists, HasTasks, Note,
    NoteRepository, Project, ProjectRepository, Repo, RepoRepository, Skill, SkillAttachment,
    SkillRepository, SyncRepository, TaskList, TaskListRepository, TaskListStatus, write_jsonl,
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
async fn test_import_restores_task_transitions() {
    // RED: Will fail because import doesn't restore transitions yet
    use context_core::{TaskRepository, TaskStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create project and task list in db1
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

    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        project_id: "projtest".to_string(),
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

    // Create task with transitions
    let task = context_core::Task {
        id: "task0001".to_string(),
        list_id: "list0001".to_string(),
        parent_id: None,
        title: "Test Task".to_string(),
        description: Some("Description".to_string()),
        status: TaskStatus::Backlog,
        priority: Some(2),
        tags: vec!["test".to_string()],
        external_refs: vec![],
        created_at: Some("2024-01-01T10:00:00Z".to_string()),
        updated_at: Some("2024-01-01T10:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.tasks().create(&task).await.unwrap();

    // Transition to in_progress
    let mut updated_task = task.clone();
    updated_task.status = TaskStatus::InProgress;
    db1.tasks().update(&updated_task).await.unwrap();

    // Export from db1
    db1.sync().export_all(temp_dir.path()).await.unwrap();

    // Import to db2
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify task was imported
    let imported_task = db2.tasks().get("task0001").await.unwrap();
    assert_eq!(imported_task.status, TaskStatus::InProgress);

    // Verify transitions were imported
    let transitions = db2
        .tasks()
        .get_transitions("task0001", None, None)
        .await
        .unwrap();
    assert_eq!(
        transitions.items.len(),
        2,
        "Should have 2 transitions: backlog -> in_progress"
    );

    // Verify statuses
    let statuses: Vec<TaskStatus> = transitions.items.iter().map(|t| t.status.clone()).collect();
    assert!(
        statuses.contains(&TaskStatus::Backlog),
        "Should have backlog transition"
    );
    assert!(
        statuses.contains(&TaskStatus::InProgress),
        "Should have in_progress transition"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_backwards_compatible_without_transitions() {
    // RED: Will fail if import requires task_transition_log.jsonl
    use context_core::{TaskRepository, TaskStatus};

    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create project and task list
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
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();

    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        project_id: "projtest".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    write_jsonl(&temp_dir.path().join("lists.jsonl"), &[task_list]).unwrap();

    // Create old-style task JSONL (without transitions file)
    let task = context_core::Task {
        id: "task0001".to_string(),
        list_id: "list0001".to_string(),
        parent_id: None,
        title: "Old Task".to_string(),
        description: Some("From old export".to_string()),
        status: TaskStatus::Done,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2024-01-01T10:00:00Z".to_string()),
        updated_at: Some("2024-01-01T12:00:00Z".to_string()),
        deleted_at: None,
    };
    write_jsonl(&temp_dir.path().join("tasks.jsonl"), &[task]).unwrap();

    // Write empty files for other entities
    write_jsonl::<Repo>(&temp_dir.path().join("repos.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();

    // NOTE: No task_transition_log.jsonl file - simulating old export

    // Import should succeed (backwards compatible)
    let result = db.sync().import_all(temp_dir.path()).await;
    assert!(
        result.is_ok(),
        "Import should succeed without transitions file (backward compatibility)"
    );

    // Verify task was imported
    let imported_task = db.tasks().get("task0001").await.unwrap();
    assert_eq!(imported_task.title, "Old Task");
    assert_eq!(imported_task.status, TaskStatus::Done);

    // Transitions should be empty (no transitions file existed)
    let transitions = db
        .tasks()
        .get_transitions("task0001", None, None)
        .await
        .unwrap();
    assert_eq!(
        transitions.items.len(),
        0,
        "Should have no transitions when importing old export"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_batch_idempotent() {
    use context_core::{Note, NoteRepository, Task, TaskRepository, TaskStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let project = Project {
        id: "projidem".to_string(),
        title: "Idempotent Project".to_string(),
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

    let repo = Repo {
        id: "repoidem".to_string(),
        remote: "https://github.com/test/idempotent".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec!["projidem".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    db1.repos().create(&repo).await.unwrap();

    let task_list = TaskList {
        id: "listidem".to_string(),
        title: "Idempotent List".to_string(),
        description: None,
        notes: None,
        project_id: "projidem".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec!["repoidem".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db1.task_lists().create(&task_list).await.unwrap();

    let task = Task {
        id: "taskidem".to_string(),
        list_id: "listidem".to_string(),
        parent_id: None,
        title: "Idempotent Task".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.tasks().create(&task).await.unwrap();

    let note = Note {
        id: "noteidem".to_string(),
        title: "Idempotent Note".to_string(),
        content: "Note content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec!["repoidem".to_string()],
        project_ids: vec!["projidem".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&note).await.unwrap();

    let skill = Skill {
        id: "skilidem".to_string(),
        name: "idempotent-skill".to_string(),
        description: "Idempotent skill".to_string(),
        content: "---\nname: idempotent-skill\ndescription: Idempotent skill\n---\n\n# Idempotent Skill\n".to_string(),
        tags: vec![],
        project_ids: vec!["projidem".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.skills().create(&skill).await.unwrap();

    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    let entities = [
        ("project", "projidem"),
        ("task_list", "listidem"),
        ("task", "taskidem"),
        ("note", "noteidem"),
        ("skill", "skilidem"),
    ];
    let mut before = Vec::new();
    for (table, id) in entities {
        before.push(crate::helpers::read_updated_at(db2.pool(), table, id).await);
    }

    // Importing the same JSONL a second time must not change any row.
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    for ((table, id), expected) in entities.iter().zip(before.iter()) {
        let after = crate::helpers::read_updated_at(db2.pool(), table, id).await;
        assert_eq!(
            &after, expected,
            "second import must not change {}.updated_at",
            table
        );
    }

    // Relationship rows must not be duplicated by the second import.
    let relationships = [
        ("project_repo", "repo_id = ?", "repoidem"),
        ("task_list_repo", "task_list_id = ?", "listidem"),
        ("project_note", "note_id = ?", "noteidem"),
        ("note_repo", "note_id = ?", "noteidem"),
        ("project_skill", "skill_id = ?", "skilidem"),
    ];
    for (table, where_clause, id) in relationships {
        let count = crate::helpers::count_rows(db2.pool(), table, where_clause, id).await;
        assert_eq!(count, 1, "{} must hold exactly one row for {}", table, id);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_lww_guard() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // The stored row is newer than the row in the JSONL file.
    let current = Project {
        id: "projlww1".to_string(),
        title: "Newer Title".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-06-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db.projects().create(&current).await.unwrap();

    let stale = Project {
        title: "Stale Title".to_string(),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        ..current.clone()
    };
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[stale]).unwrap();
    write_jsonl::<Repo>(&temp_dir.path().join("repos.jsonl"), &[]).unwrap();
    write_jsonl::<TaskList>(&temp_dir.path().join("lists.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Task>(&temp_dir.path().join("tasks.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let unchanged = db.projects().get("projlww1").await.unwrap();
    assert_eq!(
        unchanged.title, "Newer Title",
        "an older import must not overwrite newer data"
    );
    assert_eq!(
        unchanged.updated_at,
        Some("2024-06-01T00:00:00Z".to_string())
    );

    // A newer row does overwrite the stored row.
    let newer = Project {
        title: "Newest Title".to_string(),
        updated_at: Some("2024-12-01T00:00:00Z".to_string()),
        ..current.clone()
    };
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[newer]).unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let updated = db.projects().get("projlww1").await.unwrap();
    assert_eq!(updated.title, "Newest Title");
    assert_eq!(updated.updated_at, Some("2024-12-01T00:00:00Z".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn import_repo_lww_guard() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // The stored row is newer than the row in the JSONL file.
    let current = Repo {
        id: "repolww1".to_string(),
        remote: "github:newer/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2024-01-01 00:00:00".to_string()),
        updated_at: Some("2024-06-01 00:00:00".to_string()),
        deleted_at: None,
    };
    db.repos().create(&current).await.unwrap();

    let stale = Repo {
        remote: "github:stale/repo".to_string(),
        updated_at: Some("2024-01-01 00:00:00".to_string()),
        ..current.clone()
    };
    write_jsonl(&temp_dir.path().join("repos.jsonl"), &[stale]).unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let unchanged = db.repos().get("repolww1").await.unwrap();
    assert_eq!(
        unchanged.remote, "github:newer/repo",
        "an older import must not overwrite newer repo data"
    );
    assert_eq!(
        unchanged.updated_at,
        Some("2024-06-01 00:00:00".to_string())
    );

    // A newer row does overwrite the stored row.
    let newer = Repo {
        remote: "github:newest/repo".to_string(),
        updated_at: Some("2024-12-01 00:00:00".to_string()),
        ..current.clone()
    };
    write_jsonl(&temp_dir.path().join("repos.jsonl"), &[newer]).unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let updated = db.repos().get("repolww1").await.unwrap();
    assert_eq!(updated.remote, "github:newest/repo");
    assert_eq!(updated.updated_at, Some("2024-12-01 00:00:00".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn import_repo_without_updated_at_succeeds() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // A pre-migration export has no updated_at key for repos.
    let legacy = serde_json::json!({
        "id": "repoleg1",
        "remote": "github:legacy/repo",
        "path": null,
        "tags": [],
        "project_ids": [],
        "created_at": "2024-01-01 00:00:00",
        "deleted_at": null
    });
    std::fs::write(temp_dir.path().join("repos.jsonl"), format!("{}\n", legacy)).unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let imported = db.repos().get("repoleg1").await.unwrap();
    assert_eq!(imported.remote, "github:legacy/repo");
    assert_eq!(
        imported.updated_at, None,
        "a missing updated_at must import as NULL"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_batches_multiple_rows() {
    use context_core::{Note, NoteRepository, Task, TaskRepository, TaskStatus};

    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let projects: Vec<Project> = (1..=3)
        .map(|i| Project {
            id: format!("projbtc{}", i),
            title: format!("Batch Project {}", i),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            deleted_at: None,
        })
        .collect();

    let repos: Vec<Repo> = (1..=3)
        .map(|i| Repo {
            id: format!("repobtc{}", i),
            remote: format!("https://github.com/test/batch{}", i),
            path: None,
            tags: vec![],
            project_ids: vec![format!("projbtc{}", i)],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .collect();

    let task_lists: Vec<TaskList> = (1..=3)
        .map(|i| TaskList {
            id: format!("listbtc{}", i),
            title: format!("Batch List {}", i),
            description: None,
            notes: None,
            project_id: format!("projbtc{}", i),
            tags: vec![],
            status: TaskListStatus::Active,
            external_refs: vec![],
            repo_ids: vec![format!("repobtc{}", i)],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            archived_at: None,
            deleted_at: None,
        })
        .collect();

    let tasks: Vec<Task> = (1..=3)
        .map(|i| Task {
            id: format!("taskbtc{}", i),
            list_id: format!("listbtc{}", i),
            parent_id: None,
            title: format!("Batch Task {}", i),
            description: None,
            status: TaskStatus::Todo,
            priority: Some(3),
            tags: vec![],
            external_refs: vec![],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            deleted_at: None,
        })
        .collect();

    let notes: Vec<Note> = (1..=3)
        .map(|i| Note {
            id: format!("notebtc{}", i),
            title: format!("Batch Note {}", i),
            content: "Note content".to_string(),
            tags: vec![],
            parent_id: None,
            idx: None,
            repo_ids: vec![format!("repobtc{}", i)],
            project_ids: vec![format!("projbtc{}", i)],
            subnote_count: None,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            deleted_at: None,
        })
        .collect();

    let skills: Vec<Skill> = (1..=3)
        .map(|i| Skill {
            id: format!("skilbtc{}", i),
            name: format!("batch-skill-{}", i),
            description: format!("Batch skill {}", i),
            content: format!(
                "---\nname: batch-skill-{}\ndescription: Batch skill {}\n---\n\n# Batch Skill {}\n",
                i, i, i
            ),
            tags: vec![],
            project_ids: vec![format!("projbtc{}", i)],
            scripts: vec![],
            references: vec![],
            assets: vec![],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            deleted_at: None,
        })
        .collect();

    write_jsonl(&temp_dir.path().join("projects.jsonl"), &projects).unwrap();
    write_jsonl(&temp_dir.path().join("repos.jsonl"), &repos).unwrap();
    write_jsonl(&temp_dir.path().join("lists.jsonl"), &task_lists).unwrap();
    write_jsonl(&temp_dir.path().join("tasks.jsonl"), &tasks).unwrap();
    write_jsonl(&temp_dir.path().join("notes.jsonl"), &notes).unwrap();
    write_jsonl(&temp_dir.path().join("skills.jsonl"), &skills).unwrap();

    let summary = db.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(summary.projects, 3);
    assert_eq!(summary.repos, 3);
    assert_eq!(summary.task_lists, 3);
    assert_eq!(summary.tasks, 3);
    assert_eq!(summary.notes, 3);
    assert_eq!(summary.skills, 3);

    for i in 1..=3 {
        let project = db.projects().get(&format!("projbtc{}", i)).await.unwrap();
        assert_eq!(project.title, format!("Batch Project {}", i));
        assert_eq!(project.repo_ids, vec![format!("repobtc{}", i)]);

        let repo = db.repos().get(&format!("repobtc{}", i)).await.unwrap();
        assert_eq!(repo.project_ids, vec![format!("projbtc{}", i)]);

        let list = db.task_lists().get(&format!("listbtc{}", i)).await.unwrap();
        assert_eq!(list.repo_ids, vec![format!("repobtc{}", i)]);

        let task = db.tasks().get(&format!("taskbtc{}", i)).await.unwrap();
        assert_eq!(task.title, format!("Batch Task {}", i));

        let note = db.notes().get(&format!("notebtc{}", i)).await.unwrap();
        assert_eq!(note.project_ids, vec![format!("projbtc{}", i)]);
        assert_eq!(note.repo_ids, vec![format!("repobtc{}", i)]);

        let skill = db.skills().get(&format!("skilbtc{}", i)).await.unwrap();
        assert_eq!(skill.project_ids, vec![format!("projbtc{}", i)]);
    }
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
        deleted_at: None,
    };

    db.notes().create(&note).await.unwrap();

    // Verify timestamps were stored correctly
    let stored_note = db.notes().get("testid01").await.unwrap();
    assert_eq!(stored_note.created_at, Some(original_created.to_string()));
    assert_eq!(stored_note.updated_at, Some(original_updated.to_string()));

    // Export the note
    db.sync().export_all(temp_dir.path()).await.unwrap();

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
        deleted_at: None,
    };

    // Write the modified note to JSONL (simulating external modification)
    write_jsonl(&temp_dir.path().join("notes.jsonl"), &[modified_note]).unwrap();

    // Import should preserve the modified timestamp
    db.sync().import_all(temp_dir.path()).await.unwrap();

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
        created_at: Some(original_created.to_string()),
        updated_at: Some(original_updated.to_string()),
        deleted_at: None,
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
        created_at: Some(original_created.to_string()),
        updated_at: Some(modified_updated.to_string()),
        deleted_at: None,
    };

    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[modified_project]).unwrap();
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("lists.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let imported_project = db.projects().get("proj1234").await.unwrap();
    assert_eq!(
        imported_project.created_at.as_ref().unwrap(),
        original_created,
        "project created_at should be preserved"
    );
    assert_eq!(
        imported_project.updated_at.as_ref().unwrap(),
        modified_updated,
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
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
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
        created_at: Some(original_created.to_string()),
        updated_at: Some(original_updated.to_string()),
        archived_at: None,
        deleted_at: None,
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
        created_at: Some(original_created.to_string()),
        updated_at: Some(modified_updated.to_string()),
        archived_at: None,
        deleted_at: None,
    };

    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();
    write_jsonl(&temp_dir.path().join("lists.jsonl"), &[modified_list]).unwrap();
    std::fs::write(temp_dir.path().join("repos.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("tasks.jsonl"), "").unwrap();
    std::fs::write(temp_dir.path().join("notes.jsonl"), "").unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let imported_list = db.task_lists().get("list1234").await.unwrap();
    assert_eq!(
        imported_list.created_at.as_ref().unwrap(),
        original_created,
        "task_list created_at should be preserved"
    );
    assert_eq!(
        imported_list.updated_at.as_ref().unwrap(),
        modified_updated,
        "task_list updated_at should be preserved from import, not auto-generated"
    );
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
        hasher.finalize().iter().fold(String::new(), |mut acc, b| {
            use std::fmt::Write;
            write!(acc, "{:02x}", b).unwrap();
            acc
        })
    };
    let script_b64 = BASE64_STANDARD.encode(script_content);

    let reference_content = b"# Reference Doc\nSome documentation";
    let reference_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(reference_content);
        hasher.finalize().iter().fold(String::new(), |mut acc, b| {
            use std::fmt::Write;
            write!(acc, "{:02x}", b).unwrap();
            acc
        })
    };
    let reference_b64 = BASE64_STANDARD.encode(reference_content);

    let skill = Skill {
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
        deleted_at: None,
    };

    let attachments = vec![
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
            deleted_at: None,
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
            deleted_at: None,
        },
    ];

    // Write to separate JSONL files and import
    write_jsonl(
        &temp_dir.path().join("skills.jsonl"),
        std::slice::from_ref(&skill),
    )
    .unwrap();
    write_jsonl(
        &temp_dir.path().join("skills_attachments.jsonl"),
        &attachments,
    )
    .unwrap();
    db.sync().import_all(temp_dir.path()).await.unwrap();

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
    assert_eq!(
        script_att.mime_type,
        Some("text/x-shellscript".to_string()),
        "import must preserve the script attachment mime_type"
    );

    let ref_att = attachments
        .iter()
        .find(|a| a.filename == "README.md")
        .unwrap();
    assert_eq!(ref_att.type_, "reference");
    assert_eq!(ref_att.content_hash, reference_hash);
    assert_eq!(
        ref_att.mime_type,
        Some("text/markdown".to_string()),
        "import must preserve the reference attachment mime_type"
    );

    // Test 2: Re-import with changed attachment content (should update)
    let modified_script = b"#!/bin/bash\necho 'modified script'";
    let modified_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(modified_script);
        hasher.finalize().iter().fold(String::new(), |mut acc, b| {
            use std::fmt::Write;
            write!(acc, "{:02x}", b).unwrap();
            acc
        })
    };
    let modified_b64 = BASE64_STANDARD.encode(modified_script);

    let updated_attachments_export = vec![
        SkillAttachment {
            id: "at001234".to_string(),
            skill_id: "sk001234".to_string(),
            type_: "script".to_string(),
            filename: "run.sh".to_string(),
            content: modified_b64.clone(),
            content_hash: modified_hash.clone(),
            mime_type: Some("text/plain".to_string()),
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-02T10:00:00Z".to_string()),
            deleted_at: None,
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
            deleted_at: None,
        },
    ];

    write_jsonl(
        &temp_dir.path().join("skills.jsonl"),
        std::slice::from_ref(&skill),
    )
    .unwrap();
    write_jsonl(
        &temp_dir.path().join("skills_attachments.jsonl"),
        &updated_attachments_export,
    )
    .unwrap();
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify attachment updated
    let updated_attachments = db.skills().get_attachments("sk001234").await.unwrap();
    assert_eq!(updated_attachments.len(), 2);

    let updated_script = updated_attachments
        .iter()
        .find(|a| a.filename == "run.sh")
        .unwrap();
    assert_eq!(updated_script.content_hash, modified_hash);
    assert_eq!(updated_script.content, modified_b64);
    assert_eq!(
        updated_script.mime_type,
        Some("text/plain".to_string()),
        "LWW update must apply the re-imported mime_type"
    );

    // Reference should be unchanged
    let unchanged_ref = updated_attachments
        .iter()
        .find(|a| a.filename == "README.md")
        .unwrap();
    assert_eq!(unchanged_ref.content_hash, reference_hash);

    // Test 3: Re-import with the script attachment tombstoned. The live import
    // never deletes attachments absent from the file; a removal propagates only
    // as a tombstone whose updated_at is later than the stored row.
    let tombstoned_attachments_export = vec![
        SkillAttachment {
            id: "at001234".to_string(),
            skill_id: "sk001234".to_string(),
            type_: "script".to_string(),
            filename: "run.sh".to_string(),
            content: modified_b64.clone(),
            content_hash: modified_hash.clone(),
            mime_type: Some("text/x-shellscript".to_string()),
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-03T10:00:00Z".to_string()),
            deleted_at: Some("2024-01-03T10:00:00Z".to_string()),
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
            deleted_at: None,
        },
    ];

    write_jsonl(
        &temp_dir.path().join("skills.jsonl"),
        std::slice::from_ref(&skill),
    )
    .unwrap();
    write_jsonl(
        &temp_dir.path().join("skills_attachments.jsonl"),
        &tombstoned_attachments_export,
    )
    .unwrap();
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify script attachment is soft-deleted (excluded from the live set)
    let live_filenames: Vec<String> = sqlx::query_scalar(
        "SELECT filename FROM skill_attachment WHERE skill_id = ? AND deleted_at IS NULL",
    )
    .bind("sk001234")
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(live_filenames, vec!["README.md".to_string()]);
}
