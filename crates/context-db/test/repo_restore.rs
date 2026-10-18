use crate::SqliteDatabase;
use crate::helpers::{read_deleted_at, read_updated_at};
use context_core::{Database, HasRepos, Repo, RepoRepository};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

fn make_repo(id: &str, remote: &str) -> Repo {
    Repo {
        id: id.to_string(),
        remote: remote.to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_repo_clears_deleted_at() {
    let db = setup_db().await;
    let repos = db.repos();

    repos
        .create(&make_repo("reporest", "github:user/restore"))
        .await
        .expect("Create should succeed");
    repos
        .delete("reporest")
        .await
        .expect("Delete should succeed");

    repos
        .restore("reporest")
        .await
        .expect("Restore should succeed");

    assert!(
        read_deleted_at(db.pool(), "repo", "reporest")
            .await
            .is_none(),
        "restore should clear deleted_at"
    );
    repos
        .get("reporest")
        .await
        .expect("restored repo should be readable");
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_nonexistent_repo_returns_not_found() {
    let db = setup_db().await;

    let result = db.repos().restore("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "restoring a missing repo should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_live_repo_is_noop() {
    let db = setup_db().await;
    let repos = db.repos();

    repos
        .create(&make_repo("repolive", "github:user/live"))
        .await
        .expect("Create should succeed");

    repos
        .restore("repolive")
        .await
        .expect("Restoring a live repo should be a no-op");

    assert!(
        read_deleted_at(db.pool(), "repo", "repolive")
            .await
            .is_none(),
        "deleted_at should stay NULL"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_restore_bumps_updated_at() {
    let db = setup_db().await;
    let repos = db.repos();

    repos
        .create(&make_repo("reporest", "github:user/restore"))
        .await
        .expect("Create should succeed");
    repos
        .delete("reporest")
        .await
        .expect("Delete should succeed");

    repos
        .restore("reporest")
        .await
        .expect("Restore should succeed");

    let updated_at = read_updated_at(db.pool(), "repo", "reporest").await;
    assert!(
        updated_at.as_deref() > Some("2025-01-01 00:00:00"),
        "restore should bump updated_at past created_at, got: {:?}",
        updated_at
    );
}
