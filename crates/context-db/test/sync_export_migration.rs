//! Tests for the legacy repo updated_at backfill migration.

use crate::SqliteDatabase;
use context_core::{HasRepos, HasSync, Repo, RepoRepository, SyncRepository};
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread")]
async fn test_legacy_repo_stops_reexporting_after_backfill() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    let temp_dir = TempDir::new().unwrap();
    let migrator = sqlx::migrate!("data/sql/sqlite/migrations");

    // Stop before the backfill migration so the row can be inserted with
    // updated_at NULL, the shape every pre-20261018000003 database has.
    migrator.run_to(20261018000003, db.pool()).await.unwrap();

    sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind("legac001")
        .bind("github:legacy/one")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-12-01 00:00:00")
        .execute(db.pool())
        .await
        .unwrap();

    migrator.run(db.pool()).await.unwrap();

    // The schema version bump forces a full export, so the legacy row appears
    // exactly once.
    let first = db.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(
        first.repos, 1,
        "the first export after the migration is full"
    );

    // SyncManager advances the cutoff after a successful export; export_all
    // does not. Use a fixed cutoff above the legacy created_at and below now.
    db.sync()
        .set_last_synced_at("2026-01-01 00:00:00")
        .await
        .unwrap();

    let second = db.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(
        second.repos, 0,
        "a backfilled legacy repo must not be re-exported"
    );

    db.repos()
        .update(&Repo {
            id: "legac001".to_string(),
            remote: "github:legacy/one-edited".to_string(),
            path: None,
            tags: vec![],
            project_ids: vec![],
            created_at: Some("2025-12-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    let third = db.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(third.repos, 1, "an edited legacy repo must be exported");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migration_backfills_repo_updated_at_from_created_at() {
    let db = SqliteDatabase::in_memory().await.unwrap();
    let migrator = sqlx::migrate!("data/sql/sqlite/migrations");

    migrator.run_to(20261018000003, db.pool()).await.unwrap();

    for (id, remote, created_at) in [
        ("legac001", "github:legacy/one", Some("2025-12-01 00:00:00")),
        ("legac002", "github:legacy/two", Some("2026-01-15 00:00:00")),
        ("legac003", "github:legacy/three", None),
    ] {
        sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(id)
            .bind(remote)
            .bind(None::<String>)
            .bind("[]")
            .bind(created_at)
            .execute(db.pool())
            .await
            .unwrap();
    }

    migrator.run(db.pool()).await.unwrap();

    let legac001: Option<String> =
        sqlx::query_scalar("SELECT updated_at FROM repo WHERE id = 'legac001'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        legac001.as_deref(),
        Some("2025-12-01 00:00:00"),
        "updated_at must be backfilled from created_at"
    );

    let legac002: Option<String> =
        sqlx::query_scalar("SELECT updated_at FROM repo WHERE id = 'legac002'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        legac002.as_deref(),
        Some("2026-01-15 00:00:00"),
        "updated_at must be backfilled from created_at"
    );

    let legac003: Option<String> =
        sqlx::query_scalar("SELECT updated_at FROM repo WHERE id = 'legac003'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(
        legac003.is_some(),
        "a NULL created_at must fall back to datetime('now')"
    );
}
