-- Track incremental sync state.
--
-- `last_synced_at` is the cutoff for the next incremental export: only rows
-- whose `updated_at` is greater than this value are exported. NULL means the
-- next export is a full export (first sync, or a schema version change).
--
-- `last_full_sync_at` records when the last full export ran.
--
-- `schema_version` records the schema version that produced `last_synced_at`.
-- When the local schema version changes (a migration ran), the stored cutoff
-- is no longer trustworthy: it resets to NULL so the next export is a full
-- export.
--
-- The table holds exactly one row (id = 1).

CREATE TABLE IF NOT EXISTS sync_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_synced_at TEXT,
    last_full_sync_at TEXT,
    schema_version INTEGER
);

INSERT OR IGNORE INTO sync_state (id, last_synced_at, last_full_sync_at, schema_version)
VALUES (1, NULL, NULL, NULL);
