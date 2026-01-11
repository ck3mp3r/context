use crate::db::{Database, SqliteDatabase};
use crate::sync::git::{GitError, MockGitOps};
use crate::sync::manager::*;
use mockall::predicate::*;
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};
use tempfile::TempDir;

fn mock_output(code: i32, stdout: &str, stderr: &str) -> Output {
    Output {
        status: ExitStatus::from_raw(code),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
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

#[tokio::test(flavor = "multi_thread")]
async fn test_init_creates_directory_and_git_repo() {
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().join("sync");

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_init()
        .with(eq(sync_dir.clone()))
        .times(1)
        .returning(|_| Ok(mock_output(0, "Initialized", "")));

    let manager = SyncManager::with_sync_dir(mock_git, sync_dir.clone());
    manager.init(None).await.unwrap();

    assert!(sync_dir.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_init_with_remote() {
    let temp_dir = TempDir::new().unwrap();
    let sync_dir = temp_dir.path().to_path_buf();

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_init()
        .times(1)
        .returning(|_| Ok(mock_output(0, "Initialized", "")));
    // Now expects remote_get_url to check if remote already exists
    mock_git
        .expect_remote_get_url()
        .with(eq(sync_dir.clone()), eq("origin"))
        .times(1)
        .returning(|_, _| Err(GitError::GitNotFound)); // No existing remote
    mock_git
        .expect_add_remote()
        .with(
            eq(sync_dir.clone()),
            eq("origin"),
            eq("https://github.com/test/repo.git"),
        )
        .times(1)
        .returning(|_, _, _| Ok(mock_output(0, "", "")));

    let manager = SyncManager::with_sync_dir(mock_git, sync_dir);
    manager
        .init(Some("https://github.com/test/repo.git".to_string()))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_not_initialized() {
    let temp_dir = TempDir::new().unwrap();
    let db = setup_test_db().await;

    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    let status = manager.status(&db).await.unwrap();

    assert!(!status.initialized);
    assert!(status.remote_url.is_none());
    assert!(status.git_status.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_not_initialized() {
    let temp_dir = TempDir::new().unwrap();
    let db = setup_test_db().await;

    let mock_git = MockGitOps::new();
    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    let result = manager.export(&db, None, false).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SyncError::NotInitialized));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_nothing_to_commit() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_test_db().await;

    let mut mock_git = MockGitOps::new();
    // No remote configured
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Err(GitError::GitNotFound));
    // Add files succeeds
    mock_git
        .expect_add_files()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "", "")));
    // Commit fails with "nothing to commit"
    mock_git.expect_commit().times(1).returning(|_, _| {
        Err(GitError::NonZeroExit {
            code: 1,
            output: "nothing to commit, working tree clean\n".to_string(),
        })
    });

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let result = manager.export(&db, None, false).await;

    // Should succeed even though commit failed - nothing to commit is not an error
    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_initialized_clean() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_test_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Ok(mock_output(0, "https://github.com/test/repo.git\n", "")));
    mock_git
        .expect_status_porcelain()
        .returning(|_| Ok(mock_output(0, "", "")));

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let status = manager.status(&db).await.unwrap();

    assert!(status.initialized);
    assert_eq!(
        status.remote_url,
        Some("https://github.com/test/repo.git".to_string())
    );
    assert!(status.git_status.as_ref().unwrap().clean);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_initialized_dirty() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_test_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Err(GitError::GitNotFound)); // No remote
    mock_git
        .expect_status_porcelain()
        .returning(|_| Ok(mock_output(0, " M repos.jsonl\n", "")));

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let status = manager.status(&db).await.unwrap();

    assert!(status.initialized);
    assert!(status.remote_url.is_none());
    assert!(!status.git_status.as_ref().unwrap().clean);
}

// ============================================================================
// TDD Tests for push/pull parameters (RED phase - these will fail initially)
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_export_with_push_false_does_not_push() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_test_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Ok(mock_output(0, "https://github.com/test/repo.git\n", "")));
    mock_git
        .expect_add_files()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "", "")));
    mock_git
        .expect_commit()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "commit successful", "")));
    // push should NOT be called when push=false
    mock_git.expect_push().times(0);

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let result = manager.export(&db, None, false).await;

    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_with_push_true_calls_push() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_test_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Ok(mock_output(0, "https://github.com/test/repo.git\n", "")));
    mock_git
        .expect_add_files()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "", "")));
    mock_git
        .expect_commit()
        .times(1)
        .returning(|_, _| Ok(mock_output(0, "commit successful", "")));
    // push SHOULD be called when push=true
    mock_git
        .expect_push()
        .with(always(), eq("origin"), eq("main"))
        .times(1)
        .returning(|_, _, _| Ok(mock_output(0, "pushed successfully", "")));

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let result = manager.export(&db, None, true).await;

    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_with_pull_false_does_not_pull() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_test_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Ok(mock_output(0, "https://github.com/test/repo.git\n", "")));
    // pull should NOT be called when pull=false
    mock_git.expect_pull().times(0);

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let result = manager.import(&db, false).await;

    // May fail due to missing JSONL files, but that's ok - we're testing git operations
    let _ = result;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_with_pull_true_calls_pull() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_test_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Ok(mock_output(0, "https://github.com/test/repo.git\n", "")));
    // pull SHOULD be called when pull=true
    mock_git
        .expect_pull()
        .with(always(), eq("origin"), eq("main"))
        .times(1)
        .returning(|_, _, _| Ok(mock_output(0, "Already up to date", "")));

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());
    let result = manager.import(&db, true).await;

    // May fail due to missing JSONL files, but that's ok - we're testing git operations
    let _ = result;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_then_export_push_idempotent() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_test_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Ok(mock_output(0, "https://github.com/test/repo.git\n", "")));
    mock_git
        .expect_add_files()
        .times(2) // Called twice (once for each export)
        .returning(|_, _| Ok(mock_output(0, "", "")));
    mock_git
        .expect_commit()
        .times(2) // Called twice
        .returning(|_, _| Ok(mock_output(0, "commit successful", "")));
    mock_git
        .expect_push()
        .times(1) // Called only on second export with push=true
        .returning(|_, _, _| Ok(mock_output(0, "pushed successfully", "")));

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    // First export without push
    let result1 = manager.export(&db, None, false).await;
    assert!(result1.is_ok());

    // Second export with push - should work!
    let result2 = manager.export(&db, None, true).await;
    assert!(result2.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_push_twice_idempotent() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_test_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Ok(mock_output(0, "https://github.com/test/repo.git\n", "")));
    mock_git
        .expect_add_files()
        .times(2)
        .returning(|_, _| Ok(mock_output(0, "", "")));
    mock_git
        .expect_commit()
        .times(2)
        .returning(|_, _| Ok(mock_output(0, "commit successful", "")));
    mock_git
        .expect_push()
        .times(2) // Both calls push
        .returning(|_, _, _| Ok(mock_output(0, "Everything up-to-date", "")));

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    // First export with push
    let result1 = manager.export(&db, None, true).await;
    assert!(result1.is_ok());

    // Second export with push - should work (idempotent)
    let result2 = manager.export(&db, None, true).await;
    assert!(result2.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_pull_twice_idempotent() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    let db = setup_test_db().await;

    let mut mock_git = MockGitOps::new();
    mock_git
        .expect_remote_get_url()
        .returning(|_, _| Ok(mock_output(0, "https://github.com/test/repo.git\n", "")));
    mock_git
        .expect_pull()
        .times(2) // Both calls pull
        .returning(|_, _, _| Ok(mock_output(0, "Already up to date", "")));

    let manager = SyncManager::with_sync_dir(mock_git, temp_dir.path().to_path_buf());

    // First import with pull (will fail on missing files, but we test git operations)
    let _ = manager.import(&db, true).await;

    // Second import with pull - should work (idempotent)
    let _ = manager.import(&db, true).await;
}
