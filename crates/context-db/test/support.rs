//! Shared test support helpers for the SQLite repositories.

/// Read the `deleted_at` tombstone marker for a row. Test-only helper.
pub(crate) async fn read_deleted_at(
    pool: &sqlx::SqlitePool,
    table: &str,
    id: &str,
) -> Option<String> {
    use sqlx::Row;

    let sql = format!("SELECT deleted_at FROM {} WHERE id = ?", table);
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("row should exist");

    row.get("deleted_at")
}

/// Read the `updated_at` timestamp for a row. Test-only helper.
pub(crate) async fn read_updated_at(
    pool: &sqlx::SqlitePool,
    table: &str,
    id: &str,
) -> Option<String> {
    use sqlx::Row;

    let sql = format!("SELECT updated_at FROM {} WHERE id = ?", table);
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("row should exist");

    row.get("updated_at")
}

/// Count rows in `table` matching `where_clause` with a single `?` bound to `id`.
/// Test-only helper.
pub(crate) async fn count_rows(
    pool: &sqlx::SqlitePool,
    table: &str,
    where_clause: &str,
    id: &str,
) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {} WHERE {}", table, where_clause);
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("count should succeed")
}
