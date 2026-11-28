-- Add archive state to notes
--
-- Notes gain an `active`/`archived` status plus an `archived_at` timestamp,
-- mirroring task lists. Archive is a live state: it is independent of the
-- `deleted_at` tombstone, so an archived note is still exported, searched and
-- restorable through the normal soft-delete path.
--
-- Idempotency: SQLite's `ALTER TABLE ... ADD COLUMN` has no `IF NOT EXISTS`
-- form, so this migration relies on sqlx's `_sqlx_migrations` tracking to run
-- exactly once per database. Existing rows receive `status = 'active'` from the
-- column default and `archived_at = NULL`.

ALTER TABLE note ADD COLUMN status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'archived'));
ALTER TABLE note ADD COLUMN archived_at TEXT;

CREATE INDEX IF NOT EXISTS idx_note_status ON note(status);
