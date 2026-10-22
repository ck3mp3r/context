use crate::git::{GitError, MockGitOps};
use crate::manager::*;
use context_core::{Database, HasProjects, HasSync, Project, ProjectRepository, SyncRepository};
use context_db::SqliteDatabase;
use tempfile::TempDir;

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

fn mock_output(code: i32, stdout: &str, stderr: &str) -> std::process::Output {
    use std::os::unix::process::ExitStatusExt;
    std::process::Output {
        status: std::process::ExitStatus::from_raw(code),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

#[test]
fn test_is_initialized_false() {
    let temp_dir = TempDir::new().unwrap();
    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    assert!(!manager.is_initialized());
}

#[test]
fn test_is_initialized_true() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();

    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    assert!(manager.is_initialized());
}

#[test]
fn test_entity_counts_includes_skills_field() {
    // Test that EntityCounts struct has skills field
    let counts = EntityCounts {
        repos: 1,
        projects: 2,
        task_lists: 3,
        tasks: 4,
        notes: 5,
        skills: 6,
        attachments: 7,
    };

    assert_eq!(counts.repos, 1);
    assert_eq!(counts.projects, 2);
    assert_eq!(counts.task_lists, 3);
    assert_eq!(counts.tasks, 4);
    assert_eq!(counts.notes, 5);
    assert_eq!(counts.skills, 6);
    assert_eq!(counts.attachments, 7);
    assert_eq!(counts.total(), 28); // Sum of all counts (1+2+3+4+5+6+7)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_writes_sync_meta() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_add_files()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "", "")));
    mock_git
        .expect_commit()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "commit successful", "")));

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    manager.export(&db, None, false).await.unwrap();

    let meta_path = temp_dir.path().join("sync_meta.json");
    assert!(
        meta_path.exists(),
        "sync_meta.json should be written on export"
    );

    let content = std::fs::read_to_string(&meta_path).unwrap();
    let meta: SyncMeta = serde_json::from_str(&content).unwrap();
    let local_version = db.schema_version().await.unwrap();

    assert_eq!(meta.schema_version, local_version);
    assert!(
        meta.schema_version > 0,
        "schema version should be positive after migrations"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_refuses_newer_schema() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_db().await;

    let local_version = db.schema_version().await.unwrap();
    let meta = SyncMeta {
        schema_version: local_version + 1,
    };
    std::fs::write(
        temp_dir.path().join("sync_meta.json"),
        serde_json::to_string(&meta).unwrap(),
    )
    .unwrap();

    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let result = manager.import(&db, false).await;

    match result {
        Err(SyncError::SchemaVersionMismatch {
            sync_version,
            local_version: local,
        }) => {
            assert_eq!(sync_version, local_version + 1);
            assert_eq!(local, local_version);
        }
        other => panic!("Expected SchemaVersionMismatch, got {:?}", other),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_allows_older_or_equal_schema() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_db().await;

    let local_version = db.schema_version().await.unwrap();
    let meta = SyncMeta {
        schema_version: local_version,
    };
    std::fs::write(
        temp_dir.path().join("sync_meta.json"),
        serde_json::to_string(&meta).unwrap(),
    )
    .unwrap();

    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let result = manager.import(&db, false).await;

    assert!(
        result.is_ok(),
        "import should proceed for equal schema: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_without_sync_meta_proceeds() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_db().await;

    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let result = manager.import(&db, false).await;

    assert!(
        result.is_ok(),
        "import should proceed without sync_meta.json: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_advances_last_synced_at() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_db().await;

    assert!(
        db.sync().last_synced_at().await.unwrap().is_none(),
        "a fresh database has no cutoff"
    );

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_add_files()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "", "")));
    mock_git
        .expect_commit()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "commit successful", "")));

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    manager.export(&db, None, false).await.unwrap();

    let cutoff = db.sync().last_synced_at().await.unwrap();
    assert!(
        cutoff.is_some(),
        "a successful export must advance last_synced_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_does_not_advance_last_synced_at_on_failure() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_add_files()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "", "")));
    mock_git.expect_commit().times(1).returning(|_, _| {
        Err(crate::git::GitError::NonZeroExit {
            code: 128,
            output: "fatal: unable to write".to_string(),
        })
    });

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let result = manager.export(&db, None, false).await;

    assert!(
        result.is_err(),
        "the failed commit must surface as an error"
    );
    assert!(
        db.sync().last_synced_at().await.unwrap().is_none(),
        "a failed export must not advance last_synced_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_purges_before_export() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_db().await;

    // Seed a project whose tombstone is older than the retention period.
    let seed_dir = temp_dir.path().join("seed");
    std::fs::create_dir_all(&seed_dir).unwrap();
    let project = Project {
        id: "expurge1".to_string(),
        title: "Expired Tombstone".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2020-01-01 00:00:00".to_string()),
        updated_at: Some("2020-01-01 00:00:00".to_string()),
        deleted_at: Some("2020-01-01 00:00:00".to_string()),
    };
    context_core::write_jsonl(&seed_dir.join("projects.jsonl"), &[project]).unwrap();
    db.sync().import_all(&seed_dir).await.unwrap();

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_add_files()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "", "")));
    mock_git
        .expect_commit()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "commit successful", "")));

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    manager.export(&db, None, false).await.unwrap();

    let exported: Vec<Project> =
        context_core::read_jsonl(&temp_dir.path().join("projects.jsonl")).unwrap();
    assert!(
        exported.is_empty(),
        "a purged project must not appear in the export, got: {:?}",
        exported
    );
}

fn make_project(id: &str) -> Project {
    Project {
        id: id.to_string(),
        title: format!("Project {id}"),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    }
}

fn status_manager(temp_dir: &TempDir) -> SyncManager<MockGitOps> {
    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Err(GitError::GitNotFound));
    mock_git
        .expect_status_porcelain()
        .returning(|_| Ok(mock_output(0, "", "")));

    SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_tombstone_counts_none_when_not_initialized() {
    let temp_dir = TempDir::new().unwrap();
    let db = setup_db().await;
    let manager = SyncManager::with_sync_dir(MockGitOps::new(), temp_dir.path().to_path_buf());

    let status = manager.status(&db).await.unwrap();

    assert!(!status.initialized, "sync must report itself uninitialized");
    assert!(
        status.tombstone_counts.is_none(),
        "an uninitialized status must not report tombstone counts"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_tombstone_counts_counts_soft_deleted_rows() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_db().await;

    db.projects()
        .create(&make_project("live0001"))
        .await
        .expect("Create live project should succeed");
    db.projects()
        .create(&make_project("tomb0001"))
        .await
        .expect("Create project to tombstone should succeed");
    db.projects()
        .delete("tomb0001")
        .await
        .expect("Soft delete should succeed");

    let manager = status_manager(&temp_dir);
    let status = manager.status(&db).await.unwrap();

    let db_counts = status.db_counts.expect("initialized status has db counts");
    let tombstones = status
        .tombstone_counts
        .expect("initialized status has tombstone counts");
    let jsonl_counts = status
        .jsonl_counts
        .expect("initialized status has jsonl counts");

    assert_eq!(db_counts.projects, 1, "only the live project is counted");
    assert_eq!(tombstones.projects, 1, "the tombstone is counted");
    assert_eq!(jsonl_counts.projects, 0, "no export has run yet");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_tombstone_counts_zero_without_tombstones() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_db().await;

    db.projects()
        .create(&make_project("live0002"))
        .await
        .expect("Create live project should succeed");

    let manager = status_manager(&temp_dir);
    let status = manager.status(&db).await.unwrap();

    let tombstones = status
        .tombstone_counts
        .expect("initialized status has tombstone counts");

    assert_eq!(
        tombstones,
        EntityCounts {
            repos: 0,
            projects: 0,
            task_lists: 0,
            tasks: 0,
            notes: 0,
            skills: 0,
            attachments: 0,
        },
        "a database without tombstones reports zero everywhere"
    );
}
