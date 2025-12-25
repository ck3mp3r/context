//! Tests for SQLite database connection and migrations.

use crate::db::{Database, SqliteDatabase};

#[test]
fn migrate_creates_all_tables() {
    let db = SqliteDatabase::in_memory().expect("Failed to create in-memory database");

    // Run migrations
    db.migrate().expect("Migration should succeed");

    // Verify all tables exist by querying sqlite_master
    let tables: Vec<String> = db
        .with_connection(|conn| {
            let mut stmt =
                conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
        .expect("Query should succeed");

    // Core tables
    assert!(tables.contains(&"repo".to_string()), "repo table missing");
    assert!(
        tables.contains(&"project".to_string()),
        "project table missing"
    );
    assert!(
        tables.contains(&"task_list".to_string()),
        "task_list table missing"
    );
    assert!(tables.contains(&"task".to_string()), "task table missing");
    assert!(tables.contains(&"note".to_string()), "note table missing");

    // Join tables
    assert!(
        tables.contains(&"project_repo".to_string()),
        "project_repo table missing"
    );
    assert!(
        tables.contains(&"project_task_list".to_string()),
        "project_task_list table missing"
    );
    assert!(
        tables.contains(&"project_note".to_string()),
        "project_note table missing"
    );
    assert!(
        tables.contains(&"task_list_repo".to_string()),
        "task_list_repo table missing"
    );
    assert!(
        tables.contains(&"note_repo".to_string()),
        "note_repo table missing"
    );
}

#[test]
fn migrate_creates_default_project() {
    let db = SqliteDatabase::in_memory().expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    let count: i64 = db
        .with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM project WHERE title = 'Default'",
                [],
                |row| row.get(0),
            )
        })
        .expect("Query should succeed");

    assert_eq!(count, 1, "Default project should exist after migration");
}

#[test]
fn migrate_is_idempotent() {
    let db = SqliteDatabase::in_memory().expect("Failed to create in-memory database");

    // Run migrations twice
    db.migrate().expect("First migration should succeed");
    db.migrate().expect("Second migration should succeed");

    // Should still have exactly one Default project
    let count: i64 = db
        .with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM project WHERE title = 'Default'",
                [],
                |row| row.get(0),
            )
        })
        .expect("Query should succeed");

    assert_eq!(count, 1, "Should have exactly one Default project");
}

#[test]
fn migrate_creates_fts_table() {
    let db = SqliteDatabase::in_memory().expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    // FTS tables appear as tables in sqlite_master
    let fts_exists: bool = db
        .with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='note_fts'",
                [],
                |row| row.get(0),
            )
        })
        .expect("Query should succeed");

    assert!(fts_exists, "note_fts FTS table should exist");
}
