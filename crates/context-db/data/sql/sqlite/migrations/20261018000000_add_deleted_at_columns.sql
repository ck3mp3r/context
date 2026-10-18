-- Add deleted_at columns for logical deletes (tombstones)
--
-- Every entity table gains a nullable `deleted_at` timestamp. NULL means the
-- row is live; a non-NULL value marks a soft-deleted tombstone that must
-- propagate through sync instead of disappearing.
--
-- Idempotency: SQLite's `ALTER TABLE ... ADD COLUMN` has no `IF NOT EXISTS`
-- form, so this migration relies on sqlx's `_sqlx_migrations` tracking to run
-- exactly once per database. Existing rows receive `deleted_at = NULL`
-- automatically because the column is nullable with no default.

ALTER TABLE project ADD COLUMN deleted_at TEXT;
ALTER TABLE task_list ADD COLUMN deleted_at TEXT;
ALTER TABLE task ADD COLUMN deleted_at TEXT;
ALTER TABLE note ADD COLUMN deleted_at TEXT;
ALTER TABLE repo ADD COLUMN deleted_at TEXT;
ALTER TABLE skill ADD COLUMN deleted_at TEXT;
ALTER TABLE skill_attachment ADD COLUMN deleted_at TEXT;
