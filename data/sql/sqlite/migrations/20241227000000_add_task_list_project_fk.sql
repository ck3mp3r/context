-- Add Foreign Key constraint to task_list.project_id
-- This ensures referential integrity: when a project is deleted, task lists are also deleted

-- SQLite doesn't support ALTER TABLE ADD CONSTRAINT, so we need to recreate the table

-- Step 1: Create new table with FK constraint
CREATE TABLE task_list_new (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    name TEXT NOT NULL,
    description TEXT,
    notes TEXT,
    tags TEXT DEFAULT '[]',  -- JSON array
    external_ref TEXT,
    status TEXT DEFAULT 'active' CHECK(status IN ('active', 'archived')),
    project_id TEXT NOT NULL CHECK(length(project_id) == 8),
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),
    archived_at TEXT,
    FOREIGN KEY (project_id) REFERENCES project(id) ON DELETE CASCADE
);

-- Step 2: Copy data from old table
INSERT INTO task_list_new (id, name, description, notes, tags, external_ref, status, project_id, created_at, updated_at, archived_at)
SELECT id, name, description, notes, tags, external_ref, status, project_id, created_at, updated_at, archived_at
FROM task_list;

-- Step 3: Drop old table
DROP TABLE task_list;

-- Step 4: Rename new table
ALTER TABLE task_list_new RENAME TO task_list;

-- Step 5: Recreate indexes
CREATE INDEX IF NOT EXISTS idx_task_list_status ON task_list(status);
CREATE INDEX IF NOT EXISTS idx_task_list_project ON task_list(project_id);

-- Step 6: Recreate triggers
CREATE TRIGGER IF NOT EXISTS task_list_update AFTER UPDATE ON task_list BEGIN
    UPDATE task_list SET updated_at = datetime('now') WHERE id = NEW.id;
END;
