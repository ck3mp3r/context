//! Tests for SyncRepository export behavior.

use crate::SqliteDatabase;
use crate::sync::{MAX_BIND_PARAMS, read_batch, rows_per_batch};
use context_core::{
    Database, HasNotes, HasProjects, HasRepos, HasSkills, HasSync, HasTaskLists, HasTasks,
    JsonlReader, JsonlWriter, NoteRepository, Project, ProjectRepository, Repo, RepoRepository,
    Skill, SkillRepository, SyncRepository, TaskList, TaskListRepository, TaskListStatus,
    current_timestamp, read_jsonl,
};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sync_repository_export_all_interface() {
    // RED: SyncRepository doesn't have export_all
    let db = setup_test_db().await;

    let temp_dir = TempDir::new().unwrap();

    let result = db.sync().export_all(temp_dir.path()).await;
    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_creates_all_jsonl_files() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Export using sync repository
    db.sync().export_all(temp_dir.path()).await.unwrap();

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
        let file_path = temp_dir.path().join(file);
        assert!(
            file_path.exists(),
            "File {} should exist at {:?}",
            file,
            file_path
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_empty_database_returns_zero_summary() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let summary = db.sync().export_all(temp_dir.path()).await.unwrap();

    assert_eq!(summary.repos, 0, "empty database must export 0 repos");
    assert_eq!(summary.projects, 0, "empty database must export 0 projects");
    assert_eq!(
        summary.task_lists, 0,
        "empty database must export 0 task lists"
    );
    assert_eq!(summary.tasks, 0, "empty database must export 0 tasks");
    assert_eq!(
        summary.transitions, 0,
        "empty database must export 0 transitions"
    );
    assert_eq!(summary.notes, 0, "empty database must export 0 notes");
    assert_eq!(summary.skills, 0, "empty database must export 0 skills");
    assert_eq!(
        summary.attachments, 0,
        "empty database must export 0 attachments"
    );
    assert_eq!(summary.total(), 0, "empty database must export 0 entities");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_skills_file_exists_after_export() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Export (even with no skills)
    db.sync().export_all(temp_dir.path()).await.unwrap();

    // skills.jsonl should exist
    let skills_file = temp_dir.path().join("skills.jsonl");
    assert!(
        skills_file.exists(),
        "skills.jsonl should exist even when empty"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_includes_task_transitions() {
    // RED: Will fail because task_transition_log.jsonl not exported yet
    use context_core::{TaskRepository, TaskStatus};

    let db = setup_test_db().await;
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
    db.projects().create(&project).await.unwrap();

    // Create a task list
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
    db.task_lists().create(&task_list).await.unwrap();

    // Create task (will log initial backlog state)
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
    db.tasks().create(&task).await.unwrap();

    // Transition the task to in_progress
    let mut updated_task = task.clone();
    updated_task.status = TaskStatus::InProgress;
    db.tasks().update(&updated_task).await.unwrap();

    // Transition to done
    updated_task.status = TaskStatus::Done;
    db.tasks().update(&updated_task).await.unwrap();

    // Export
    let export_summary = db.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.tasks, 1, "Should export 1 task");

    // Verify task_transition_log.jsonl exists
    let transitions_file = temp_dir.path().join("task_transition_log.jsonl");
    assert!(
        transitions_file.exists(),
        "task_transition_log.jsonl should exist after export"
    );

    // Read and verify transitions
    use context_core::TransitionLog;
    let transitions: Vec<TransitionLog> = context_core::read_jsonl(&transitions_file).unwrap();

    assert_eq!(
        transitions.len(),
        3,
        "Should have 3 transitions: backlog -> in_progress -> done"
    );

    // Verify all transitions are for our task
    for transition in &transitions {
        assert_eq!(
            transition.task_id, "task0001",
            "All transitions should be for task0001"
        );
    }

    // Verify statuses (order not guaranteed, but presence is)
    let statuses: Vec<TaskStatus> = transitions.iter().map(|t| t.status.clone()).collect();
    assert!(
        statuses.contains(&TaskStatus::Backlog),
        "Should have backlog transition"
    );
    assert!(
        statuses.contains(&TaskStatus::InProgress),
        "Should have in_progress transition"
    );
    assert!(
        statuses.contains(&TaskStatus::Done),
        "Should have done transition"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_transitions_single_query() {
    use context_core::{TaskRepository, TaskStatus, TransitionLog};

    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

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
    db.projects().create(&project).await.unwrap();

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
    db.task_lists().create(&task_list).await.unwrap();

    // Create 3 tasks, each with 3 transitions (backlog -> in_progress -> done)
    for index in 1..=3 {
        let task = context_core::Task {
            id: format!("task000{}", index),
            list_id: "list0001".to_string(),
            parent_id: None,
            title: format!("Task {}", index),
            description: None,
            status: TaskStatus::Backlog,
            priority: None,
            tags: vec![],
            external_refs: vec![],
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            updated_at: Some("2024-01-01T10:00:00Z".to_string()),
            deleted_at: None,
        };
        db.tasks().create(&task).await.unwrap();

        let mut updated = task.clone();
        updated.status = TaskStatus::InProgress;
        db.tasks().update(&updated).await.unwrap();

        updated.status = TaskStatus::Done;
        db.tasks().update(&updated).await.unwrap();
    }

    let export_summary = db.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.tasks, 3, "Should export 3 tasks");
    assert_eq!(
        export_summary.transitions, 9,
        "Should export 9 transitions (3 per task)"
    );

    let transitions_file = temp_dir.path().join("task_transition_log.jsonl");
    let transitions: Vec<TransitionLog> = context_core::read_jsonl(&transitions_file).unwrap();
    assert_eq!(
        transitions.len(),
        9,
        "JSONL should contain all 9 transitions"
    );

    for index in 1..=3 {
        let task_id = format!("task000{}", index);
        let count = transitions
            .iter()
            .filter(|transition| transition.task_id == task_id)
            .count();
        assert_eq!(count, 3, "task {} should have 3 transitions", task_id);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_includes_deleted_at_and_import_preserves_tombstone() {
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let project = Project {
        id: "projtomb".to_string(),
        title: "Tombstone Project".to_string(),
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
    db1.projects().delete("projtomb").await.unwrap();

    db1.sync().export_all(temp_dir.path()).await.unwrap();

    let contents = std::fs::read_to_string(temp_dir.path().join("projects.jsonl"))
        .expect("projects.jsonl should exist");
    let line = contents
        .lines()
        .find(|line| line.contains("projtomb"))
        .expect("soft-deleted project should still be exported as a tombstone");
    let value: serde_json::Value = serde_json::from_str(line).unwrap();
    assert!(
        value["deleted_at"].is_string(),
        "export must include a non-null deleted_at, got: {}",
        line
    );

    db2.sync().import_all(temp_dir.path()).await.unwrap();

    let deleted_at = crate::helpers::read_deleted_at(db2.pool(), "project", "projtomb").await;
    assert!(
        deleted_at.is_some(),
        "import must preserve the deleted_at tombstone"
    );

    let result = db2.projects().get("projtomb").await;
    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "an imported tombstone must not be visible via get, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_excludes_transitions_of_tombstoned_tasks() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    sqlx::query(
        "INSERT INTO project (id, title, tags, created_at, updated_at)
         VALUES ('projtrns', 'Transitions', '[]', datetime('now'), datetime('now'))",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO task_list (id, project_id, title, status, tags, created_at, updated_at)
         VALUES ('listtrns', 'projtrns', 'List', 'active', '[]', datetime('now'), datetime('now'))",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO task (id, list_id, title, status, tags, external_refs, created_at, updated_at)
         VALUES ('tasktrns', 'listtrns', 'Task', 'backlog', '[]', '[]', datetime('now'), datetime('now'))",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO task_transition_log (id, task_id, status, transitioned_at)
         VALUES ('trns0001', 'tasktrns', 'backlog', datetime('now'))",
    )
    .execute(db.pool())
    .await
    .unwrap();

    // Soft-deleting the project cascades to the task list and task but
    // leaves the transition row in place.
    db.projects().delete("projtrns").await.unwrap();

    db.sync().export_all(temp_dir.path()).await.unwrap();

    let contents = std::fs::read_to_string(temp_dir.path().join("task_transition_log.jsonl"))
        .expect("task_transition_log.jsonl should exist");
    assert!(
        !contents.contains("trns0001"),
        "transitions of tombstoned tasks must not be exported, got: {}",
        contents
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_uses_direct_sql() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let project = Project {
        id: "projdir1".to_string(),
        title: "Direct SQL Project".to_string(),
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

    let repo = Repo {
        id: "repodir1".to_string(),
        remote: "https://github.com/test/direct".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec!["projdir1".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    db.repos().create(&repo).await.unwrap();

    let task_list = TaskList {
        id: "listdir1".to_string(),
        title: "Direct SQL List".to_string(),
        description: None,
        notes: None,
        project_id: "projdir1".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec!["repodir1".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db.task_lists().create(&task_list).await.unwrap();

    let note = context_core::Note {
        id: "notedir1".to_string(),
        title: "Direct SQL Note".to_string(),
        content: "content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec!["repodir1".to_string()],
        project_ids: vec!["projdir1".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db.notes().create(&note).await.unwrap();

    let skill = Skill {
        id: "skilldir".to_string(),
        name: "direct-sql-skill".to_string(),
        description: "A skill".to_string(),
        content: "---\nname: direct-sql-skill\ndescription: A skill\n---\n\n# Direct SQL Skill\n"
            .to_string(),
        tags: vec![],
        project_ids: vec!["projdir1".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db.skills().create(&skill).await.unwrap();

    db.sync().export_all(temp_dir.path()).await.unwrap();

    let read_line = |file: &str, id: &str| -> serde_json::Value {
        let contents = std::fs::read_to_string(temp_dir.path().join(file))
            .unwrap_or_else(|e| panic!("{} should exist: {}", file, e));
        let line = contents
            .lines()
            .find(|line| line.contains(id))
            .unwrap_or_else(|| panic!("{} should contain {}", file, id));
        serde_json::from_str(line).unwrap()
    };

    let repo_json = read_line("repos.jsonl", "repodir1");
    assert_eq!(repo_json["project_ids"], serde_json::json!(["projdir1"]));

    let project_json = read_line("projects.jsonl", "projdir1");
    assert_eq!(project_json["repo_ids"], serde_json::json!(["repodir1"]));
    assert_eq!(
        project_json["task_list_ids"],
        serde_json::json!(["listdir1"])
    );
    assert_eq!(project_json["note_ids"], serde_json::json!(["notedir1"]));

    let list_json = read_line("lists.jsonl", "listdir1");
    assert_eq!(list_json["repo_ids"], serde_json::json!(["repodir1"]));

    let note_json = read_line("notes.jsonl", "notedir1");
    assert_eq!(note_json["repo_ids"], serde_json::json!(["repodir1"]));
    assert_eq!(note_json["project_ids"], serde_json::json!(["projdir1"]));

    let skill_json = read_line("skills.jsonl", "skilldir");
    assert_eq!(skill_json["project_ids"], serde_json::json!(["projdir1"]));

    // An entity with zero M:N relationships must export an empty array, not null.
    let orphan = Repo {
        id: "repozero".to_string(),
        remote: "https://github.com/test/orphan".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    db.repos().create(&orphan).await.unwrap();
    db.sync().export_all(temp_dir.path()).await.unwrap();

    let orphan_json = read_line("repos.jsonl", "repozero");
    assert_eq!(orphan_json["project_ids"], serde_json::json!([]));
}

// ========== Phase 3: Incremental export ==========

fn make_project(id: &str, title: &str, updated_at: &str) -> Project {
    Project {
        id: id.to_string(),
        title: title.to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01 00:00:00".to_string()),
        updated_at: Some(updated_at.to_string()),
        deleted_at: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_incremental_export_filters_by_updated_at() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    db.projects()
        .create(&make_project(
            "old00001",
            "Old Project",
            "2024-01-01 00:00:00",
        ))
        .await
        .unwrap();
    db.projects()
        .create(&make_project(
            "new00001",
            "New Project",
            "2024-06-01 00:00:00",
        ))
        .await
        .unwrap();

    db.sync()
        .set_last_synced_at("2024-03-01 00:00:00")
        .await
        .unwrap();

    db.sync().export_all(temp_dir.path()).await.unwrap();

    let projects: Vec<Project> = read_jsonl(&temp_dir.path().join("projects.jsonl")).unwrap();
    assert_eq!(
        projects.len(),
        1,
        "only the project newer than the cutoff should be exported"
    );
    assert_eq!(projects[0].id, "new00001");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_incremental_export_includes_tombstones() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    db.projects()
        .create(&make_project(
            "tomb0001",
            "Doomed Project",
            "2024-01-01 00:00:00",
        ))
        .await
        .unwrap();

    // The cutoff sits between the create and the delete.
    db.sync()
        .set_last_synced_at("2024-03-01 00:00:00")
        .await
        .unwrap();

    db.projects().delete("tomb0001").await.unwrap();

    db.sync().export_all(temp_dir.path()).await.unwrap();

    let projects: Vec<Project> = read_jsonl(&temp_dir.path().join("projects.jsonl")).unwrap();
    assert_eq!(projects.len(), 1, "the tombstone should be exported");
    assert_eq!(projects[0].id, "tomb0001");
    assert!(
        projects[0].deleted_at.is_some(),
        "exported tombstone must carry deleted_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn export_includes_restored_repo() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let repo = Repo {
        id: "reporex1".to_string(),
        remote: "https://github.com/test/restored".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2024-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    db.repos().create(&repo).await.unwrap();

    // The cutoff sits after the create, before the delete and the restore.
    db.sync()
        .set_last_synced_at("2024-03-01 00:00:00")
        .await
        .unwrap();

    db.repos().delete("reporex1").await.unwrap();
    db.repos().restore("reporex1").await.unwrap();

    db.sync().export_all(temp_dir.path()).await.unwrap();

    let repos: Vec<Repo> = read_jsonl(&temp_dir.path().join("repos.jsonl")).unwrap();
    assert_eq!(
        repos.len(),
        1,
        "a restored repo must be exported incrementally"
    );
    assert_eq!(repos[0].id, "reporex1");
    assert!(
        repos[0].deleted_at.is_none(),
        "the exported restored repo must be live"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_incremental_export_empty_on_no_changes() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    db.projects()
        .create(&make_project(
            "noch0001",
            "Unchanged Project",
            "2024-01-01 00:00:00",
        ))
        .await
        .unwrap();

    db.sync().export_all(temp_dir.path()).await.unwrap();

    // Advance the cutoff past every row, then export again with no changes.
    db.sync()
        .set_last_synced_at(&current_timestamp())
        .await
        .unwrap();

    db.sync().export_all(temp_dir.path()).await.unwrap();

    for file in [
        "repos.jsonl",
        "projects.jsonl",
        "lists.jsonl",
        "tasks.jsonl",
        "task_transition_log.jsonl",
        "notes.jsonl",
        "skills.jsonl",
        "skills_attachments.jsonl",
    ] {
        let content = std::fs::read_to_string(temp_dir.path().join(file)).unwrap();
        assert!(
            content.trim().is_empty(),
            "{} should be empty on a no-change export, got: {}",
            file,
            content
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sync_cutoff_resets_on_schema_version_change() {
    let db = setup_test_db().await;

    db.sync()
        .set_last_synced_at("2024-03-01 00:00:00")
        .await
        .unwrap();
    assert_eq!(
        db.sync().last_synced_at().await.unwrap(),
        Some("2024-03-01 00:00:00".to_string())
    );

    // Simulate a migration by recording an older schema version.
    sqlx::query("UPDATE sync_state SET schema_version = 1 WHERE id = 1")
        .execute(db.pool())
        .await
        .unwrap();

    assert_eq!(
        db.sync().last_synced_at().await.unwrap(),
        None,
        "a schema version change must reset the cutoff"
    );
}

#[test]
fn test_rows_per_batch_stays_under_bind_limit() {
    for columns in [2, 4, 6, 8, 9, 10, 12] {
        let rows = rows_per_batch(columns);
        assert!(rows >= 1, "columns={} must allow at least one row", columns);
        assert!(
            rows * columns <= MAX_BIND_PARAMS,
            "columns={} rows={} exceeds the bind limit",
            columns,
            rows
        );
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Item {
    id: String,
}

#[test]
fn test_read_batch_reads_in_chunks() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("items.jsonl");

    let mut writer = JsonlWriter::create(&path).unwrap();
    for i in 0..5 {
        writer.write(&Item { id: i.to_string() }).unwrap();
    }
    writer.finish().unwrap();

    let mut reader = JsonlReader::open(&path).unwrap();
    let first: Vec<Item> = read_batch(&mut reader, 2).unwrap();
    let second: Vec<Item> = read_batch(&mut reader, 2).unwrap();
    let third: Vec<Item> = read_batch(&mut reader, 2).unwrap();
    let fourth: Vec<Item> = read_batch(&mut reader, 2).unwrap();

    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 2);
    assert_eq!(third.len(), 1);
    assert!(fourth.is_empty());
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
