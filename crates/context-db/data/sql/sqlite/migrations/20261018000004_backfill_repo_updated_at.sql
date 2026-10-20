-- Backfill `repo.updated_at` for rows that predate the column.
--
-- 20261018000003 added `updated_at` as a nullable column with no default, so
-- every repo row that existed before that migration carries `updated_at =
-- NULL`. The incremental export predicate is
-- `WHERE r.updated_at IS NULL OR r.updated_at > ?`, so those rows bypass the
-- cutoff and are re-exported on every sync run.
--
-- Backfilling from `created_at` keeps the rows below any live cutoff: after
-- the one full export forced by the schema version bump, they stop appearing.
-- A fixed `now()` would sit above the cutoff and flood one export instead.
-- `datetime('now')` covers rows with a NULL `created_at`; the base schema
-- defaults that column, so such a row is unreachable through normal writes.
--
-- Idempotency: the UPDATE touches only rows with `updated_at IS NULL`, and
-- sqlx runs each migration exactly once per database.

UPDATE repo SET updated_at = COALESCE(created_at, datetime('now')) WHERE updated_at IS NULL;
