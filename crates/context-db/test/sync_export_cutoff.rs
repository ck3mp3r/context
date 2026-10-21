//! Tests for SyncRepository export cutoff and tombstone behavior.

use crate::SqliteDatabase;
use context_core::{
    Database, HasProjects, HasRepos, HasSync, Project, ProjectRepository, Repo, RepoRepository,
    SyncRepository, current_timestamp, read_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

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

    // The cutoff equals the older project's updated_at, so that row is not a
    // change. It must still be exported: the file is a full replica.
    db.sync()
        .set_last_synced_at("2024-01-01 00:00:00")
        .await
        .unwrap();

    let summary = db.sync().export_all(temp_dir.path()).await.unwrap();

    let projects: Vec<Project> = read_jsonl(&temp_dir.path().join("projects.jsonl")).unwrap();
    assert_eq!(
        projects.len(),
        2,
        "every project must be exported regardless of the cutoff"
    );
    assert!(
        projects.iter().any(|project| project.id == "old00001"),
        "a project whose updated_at equals the cutoff must still be exported"
    );
    assert!(
        projects.iter().any(|project| project.id == "new00001"),
        "a project newer than the cutoff must be exported"
    );

    assert_eq!(
        summary.projects, 1,
        "only the project newer than the cutoff counts as changed"
    );
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

    let summary = db.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(
        summary.total(),
        0,
        "a no-change export must report zero changed rows"
    );

    // The files are a full replica, so the unchanged project is still there.
    let projects: Vec<Project> = read_jsonl(&temp_dir.path().join("projects.jsonl")).unwrap();
    assert_eq!(
        projects.len(),
        1,
        "a no-change export must keep the full dataset in projects.jsonl"
    );
    assert_eq!(projects[0].id, "noch0001");

    for file in [
        "repos.jsonl",
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
            "{} has no rows in this dataset, got: {}",
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
