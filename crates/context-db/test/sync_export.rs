//! Tests for SyncRepository export behavior.

use crate::SqliteDatabase;
use crate::sync::{MAX_BIND_PARAMS, read_batch, rows_per_batch};
use context_core::{
    Database, HasNotes, HasProjects, HasRepos, HasSkills, HasSync, HasTaskLists, JsonlReader,
    JsonlWriter, NoteRepository, Project, ProjectRepository, Repo, RepoRepository, Skill,
    SkillRepository, SyncRepository, TaskList, TaskListRepository, TaskListStatus,
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
        source: None,
        source_subpath: None,
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
