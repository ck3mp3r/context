-- Add skill import source columns
--
-- A skill imported from a git URL or a local path records where it came from so
-- a later refresh can re-fetch the same source. Both columns are nullable TEXT
-- with no default: NULL means the source is unknown, which covers skills
-- created directly through the API and rows imported from an export that
-- predates these columns.
--
-- Idempotency: SQLite's `ALTER TABLE ... ADD COLUMN` has no `IF NOT EXISTS`
-- form, so this migration relies on sqlx's `_sqlx_migrations` tracking to run
-- exactly once per database.

ALTER TABLE skill ADD COLUMN source TEXT;
ALTER TABLE skill ADD COLUMN source_subpath TEXT;
