-- Add note.parent_id FOREIGN KEY constraint
--
-- The note.parent_id column (added in 20260110000000) was created without a
-- declared FOREIGN KEY, so referential integrity and ON DELETE CASCADE for
-- note hierarchies were never enforced. SQLite cannot ALTER TABLE to add a
-- foreign key, so we rebuild the note table.
--
-- NOTE: SQLite only enforces foreign keys when PRAGMA foreign_keys = ON. This
-- app currently does not enable that pragma, so enforcement is only effective
-- once the application enables it (matching how task.parent_id behaves today).

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

-- Copy existing data.
INSERT INTO note_new (id, title, content, tags, parent_id, idx, created_at, updated_at)
SELECT id, title, content, tags, parent_id, idx, created_at, updated_at FROM note;

-- Drop old table and rename new one into place.
DROP TABLE note;
ALTER TABLE note_new RENAME TO note;

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
