-- Add note.parent_id FOREIGN KEY constraint
--
-- The note.parent_id column (added in 20260110000000) was created without a
-- declared FOREIGN KEY, so referential integrity and ON DELETE CASCADE for
-- note hierarchies were never enforced. SQLite cannot ALTER TABLE to add a
-- foreign key, so we rebuild the note table.
--
-- This migration preserves M:N association rows (project_note, note_repo)
-- and FTS indices across the table rebuild.
--
-- Three cascade-delete hazards exist when DROP TABLE note runs with
-- foreign_keys(true):
--   1. project_note has FK(note_id) REFERENCES note(id) ON DELETE CASCADE
--   2. note_repo has FK(note_id) REFERENCES note(id) ON DELETE CASCADE
--   3. note_new has FK(parent_id) REFERENCES note(id) ON DELETE CASCADE —
--      while the old note table still exists, dropping note cascade-deletes
--      note_new rows whose parent_id is non-null.
-- We back up all three datasets and restore them after the rename.

-- Back up M:N association tables and parent_id values before dropping note.
CREATE TABLE _project_note_backup AS SELECT * FROM project_note;
CREATE TABLE _note_repo_backup AS SELECT * FROM note_repo;
CREATE TABLE _parent_id_backup AS SELECT id, parent_id FROM note;

-- Drop indexes and FTS triggers that reference the note table first.
DROP INDEX IF EXISTS idx_note_parent_id;
DROP INDEX IF EXISTS idx_note_parent_idx;
DROP INDEX IF EXISTS idx_note_parent_updated;
DROP INDEX IF EXISTS idx_note_updated_at;

DROP TRIGGER IF EXISTS note_ai;
DROP TRIGGER IF EXISTS note_au;
DROP TRIGGER IF EXISTS note_ad;

-- Rebuild note with the parent_id FK.
CREATE TABLE note_new (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT DEFAULT '[]',
    parent_id TEXT CHECK(parent_id IS NULL OR length(parent_id) == 8),
    idx INTEGER,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (parent_id) REFERENCES note(id) ON DELETE CASCADE
);

-- Null out orphaned parent_id values before applying FK constraint.
-- Parent notes were deleted while FKs were unenforced, leaving children
-- pointing to non-existent IDs. With foreign_keys(true) enabled, the
-- INSERT below would fail on these rows.
UPDATE note SET parent_id = NULL WHERE parent_id IS NOT NULL AND parent_id NOT IN (SELECT id FROM note);

-- Copy existing data, preserving rowids so the FTS index stays valid.
INSERT INTO note_new (rowid, id, title, content, tags, parent_id, idx, created_at, updated_at)
SELECT rowid, id, title, content, tags, parent_id, idx, created_at, updated_at FROM note;

-- Null out parent_id in note_new so DROP TABLE note does not cascade-delete
-- note_new rows through the note_new.parent_id FK.
UPDATE note_new SET parent_id = NULL;

-- Drop old table and rename new one into place.
-- This cascade-deletes project_note and note_repo rows, but we backed them up.
DROP TABLE note;
ALTER TABLE note_new RENAME TO note;

-- Restore parent_id from backup. Skip orphans (parent not in note table).
UPDATE note SET parent_id = (
    SELECT parent_id FROM _parent_id_backup
    WHERE _parent_id_backup.id = note.id
) WHERE note.id IN (
    SELECT id FROM _parent_id_backup
    WHERE parent_id IS NOT NULL
      AND parent_id IN (SELECT id FROM note)
);

-- Restore M:N associations from backup.
INSERT INTO project_note SELECT * FROM _project_note_backup;
INSERT INTO note_repo SELECT * FROM _note_repo_backup;

-- Clean up backup tables.
DROP TABLE _project_note_backup;
DROP TABLE _note_repo_backup;
DROP TABLE _parent_id_backup;

-- Rebuild FTS index in case rowids changed during table rebuild.
INSERT INTO note_fts(note_fts) VALUES('rebuild');

-- Recreate indexes.
CREATE INDEX IF NOT EXISTS idx_note_parent_id ON note(parent_id);
CREATE INDEX IF NOT EXISTS idx_note_parent_idx ON note(parent_id, idx);
CREATE INDEX IF NOT EXISTS idx_note_parent_updated ON note(parent_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_note_updated_at ON note(updated_at);

-- Recreate FTS sync triggers.
CREATE TRIGGER IF NOT EXISTS note_ai AFTER INSERT ON note BEGIN
    INSERT INTO note_fts(rowid, title, content, tags)
    VALUES (new.rowid, new.title, new.content, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS note_au AFTER UPDATE ON note
WHEN old.title != new.title OR old.content != new.content OR old.tags != new.tags BEGIN
    INSERT INTO note_fts(note_fts, rowid, title, content, tags)
    VALUES('delete', old.rowid, old.title, old.content, old.tags);
    INSERT INTO note_fts(rowid, title, content, tags)
    VALUES (new.rowid, new.title, new.content, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS note_ad AFTER DELETE ON note BEGIN
    INSERT INTO note_fts(note_fts, rowid, title, content, tags)
    VALUES('delete', old.rowid, old.title, old.content, old.tags);
END;
