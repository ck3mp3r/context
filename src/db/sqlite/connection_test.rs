//! Tests for SQLite database connection and migrations.

use crate::db::{Database, SqliteDatabase};

#[tokio::test(flavor = "multi_thread")]
async fn migrate_creates_all_tables() {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");

    // Run migrations
    db.migrate().expect("Migration should succeed");

    // Verify all tables exist by querying sqlite_master
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(db.pool())
            .await
            .expect("Query should succeed");

    // Expected tables from schema
    // Note: FTS5 creates internal tables (config, data, docsize, idx) but not content table
    // for contentless FTS. sqlite_sequence only exists if AUTOINCREMENT is used.
    // _sqlx_migrations is created by sqlx for migration tracking.
    let expected = vec![
        "_sqlx_migrations",
        "note",
        "note_fts",
        "note_fts_config",
        "note_fts_data",
        "note_fts_docsize",
        "note_fts_idx",
        "note_repo",
        "project",
        "project_note",
        "project_repo",
        "repo",
        "task",
        "task_list",
        "task_list_repo",
    ];

    // Check that all expected tables exist
    for table in &expected {
        assert!(
            tables.iter().any(|t| t == table),
            "Missing table: {}. Found tables: {:?}",
            table,
            tables
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_creates_tables() {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    // Verify core tables exist
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('project', 'task', 'task_list', 'note', 'repo')"
    )
    .fetch_all(db.pool())
    .await
    .expect("Query should succeed");

    assert_eq!(tables.len(), 5, "Should have 5 core tables");
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_is_idempotent() {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");

    // Run migrations twice
    db.migrate().expect("First migration should succeed");
    db.migrate().expect("Second migration should succeed");

    // Verify tables still exist
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='project'",
    )
    .fetch_one(db.pool())
    .await
    .expect("Query should succeed");

    assert_eq!(count, 1, "Project table should exist exactly once");
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_creates_fts_table() {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    // FTS tables appear as tables in sqlite_master
    let fts_exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='note_fts'",
    )
    .fetch_one(db.pool())
    .await
    .expect("Query should succeed");

    assert!(fts_exists, "note_fts FTS table should exist");
}
