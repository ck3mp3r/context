-- Add an updated_at column to repo so repo edits, deletes, and restores
-- propagate through incremental sync.
--
-- `repo` was the only entity table without an `updated_at` column. Without it
-- the incremental export could only compare the delete time (falling back to
-- the creation time) against the sync cutoff, so a restored repo
-- (`deleted_at = NULL`) fell back to its old `created_at` and was never
-- exported again. The import upsert could not carry a last-write-wins guard
-- either.
--
-- Idempotency: SQLite's `ALTER TABLE ... ADD COLUMN` has no `IF NOT EXISTS`
-- form, so this migration relies on sqlx's `_sqlx_migrations` tracking to run
-- exactly once per database. Existing rows receive `updated_at = NULL`
-- automatically because the column is nullable with no default.

ALTER TABLE repo ADD COLUMN updated_at TEXT;
