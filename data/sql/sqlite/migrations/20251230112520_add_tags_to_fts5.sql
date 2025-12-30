-- Add tags column to FTS5 search index
-- This migration updates the note_fts virtual table to include tags for full-text search

-- Drop existing FTS5 table and triggers
DROP TRIGGER IF EXISTS note_ad;
DROP TRIGGER IF EXISTS note_au;
DROP TRIGGER IF EXISTS note_ai;
DROP TABLE IF EXISTS note_fts;

-- Recreate FTS5 table with tags column
CREATE VIRTUAL TABLE note_fts USING fts5(
    title,
    content,
    tags,
    content='note',
    content_rowid='rowid'
);

-- Recreate triggers with tags support
CREATE TRIGGER note_ai AFTER INSERT ON note BEGIN
    INSERT INTO note_fts(rowid, title, content, tags) 
    VALUES (new.rowid, new.title, new.content, new.tags);
END;

CREATE TRIGGER note_au AFTER UPDATE ON note 
WHEN old.title != new.title OR old.content != new.content OR old.tags != new.tags BEGIN
    INSERT INTO note_fts(note_fts, rowid, title, content, tags) 
    VALUES('delete', old.rowid, old.title, old.content, old.tags);
    INSERT INTO note_fts(rowid, title, content, tags) 
    VALUES (new.rowid, new.title, new.content, new.tags);
END;

CREATE TRIGGER note_ad AFTER DELETE ON note BEGIN
    INSERT INTO note_fts(note_fts, rowid, title, content, tags) 
    VALUES('delete', old.rowid, old.title, old.content, old.tags);
END;

-- Rebuild FTS5 index from existing note data
INSERT INTO note_fts(rowid, title, content, tags)
SELECT rowid, title, content, tags FROM note;
