-- Add updated_at column to task table
-- Migration: 20251231132607_add_task_updated_at.sql

-- Disable foreign keys during migration (prevent cascade issues)
PRAGMA foreign_keys = OFF;

-- Step 1: Add the column (initially NULL)
ALTER TABLE task ADD COLUMN updated_at TEXT;

-- Step 2: Backfill updated_at for existing tasks
-- For tasks WITH children: use the max timestamp from all children
-- For tasks WITHOUT children: use the newest of created_at, started_at, completed_at
UPDATE task
SET updated_at = (
    SELECT COALESCE(
        -- If task has children, use max child timestamp
        (SELECT MAX(
            COALESCE(
                completed_at,
                started_at,
                created_at
            )
        )
        FROM task AS child
        WHERE child.parent_id = task.id),
        
        -- Otherwise use this task's newest timestamp
        COALESCE(
            completed_at,
            started_at,
            created_at
        )
    )
);

-- Step 3: Drop FTS table BEFORE rebuilding task table (prevents ROWID corruption)
DROP TABLE IF EXISTS task_fts;
DROP TRIGGER IF EXISTS task_fts_insert;
DROP TRIGGER IF EXISTS task_fts_delete;
DROP TRIGGER IF EXISTS task_fts_update;

-- Step 4: Make the column NOT NULL and add index
-- SQLite doesn't support ALTER COLUMN, so we rebuild the table
CREATE TABLE task_new (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    list_id TEXT NOT NULL,
    parent_id TEXT,
    title TEXT NOT NULL CHECK(length(title) <= 500),
    description TEXT CHECK(description IS NULL OR length(description) <= 10000),
    status TEXT NOT NULL CHECK(status IN ('backlog', 'todo', 'in_progress', 'review', 'done', 'cancelled')),
    priority INTEGER CHECK(priority BETWEEN 1 AND 5),
    tags TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags)),
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (list_id) REFERENCES task_list(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_id) REFERENCES task(id) ON DELETE CASCADE
);

-- Copy data
INSERT INTO task_new 
SELECT id, list_id, parent_id, title, description, status, priority, tags, 
       created_at, started_at, completed_at, updated_at
FROM task;

-- Drop old table and rename
DROP TABLE task;
ALTER TABLE task_new RENAME TO task;

-- Recreate indexes
CREATE INDEX idx_task_list_id ON task(list_id);
CREATE INDEX idx_task_parent_id ON task(parent_id);
CREATE INDEX idx_task_status ON task(status);
CREATE INDEX idx_task_priority ON task(priority);
CREATE INDEX idx_task_created_at ON task(created_at);
CREATE INDEX idx_task_updated_at ON task(updated_at);

-- Recreate FTS5 virtual table (after task table is rebuilt)
-- NOTE: Removing content=task to avoid ROWID corruption after table rebuild
CREATE VIRTUAL TABLE task_fts USING fts5(
    id UNINDEXED,
    title,
    description,
    tags
);

-- Populate FTS5 BEFORE creating triggers (avoid double-insert)
INSERT INTO task_fts(id, title, description, tags)
SELECT id, title, description, tags FROM task;

-- Recreate FTS5 triggers (AFTER population)
CREATE TRIGGER task_fts_insert AFTER INSERT ON task BEGIN
    INSERT INTO task_fts(id, title, description, tags)
    VALUES (new.id, new.title, new.description, new.tags);
END;

CREATE TRIGGER task_fts_delete AFTER DELETE ON task BEGIN
    DELETE FROM task_fts WHERE id = old.id;
END;

CREATE TRIGGER task_fts_update AFTER UPDATE ON task BEGIN
    DELETE FROM task_fts WHERE id = old.id;
    INSERT INTO task_fts(id, title, description, tags)
    VALUES (new.id, new.title, new.description, new.tags);
END;

-- Trigger to cascade updated_at to parent when subtask is updated
-- No recursion: only fires when child (parent_id NOT NULL) is updated
-- Parent update won't trigger again (parent has parent_id = NULL)
-- Parent's updated_at is set to child's updated_at
CREATE TRIGGER task_cascade_updated_at_to_parent AFTER UPDATE ON task
WHEN new.parent_id IS NOT NULL
BEGIN
    UPDATE task 
    SET updated_at = new.updated_at
    WHERE id = new.parent_id;
END;

-- Re-enable foreign keys
PRAGMA foreign_keys = ON;
