-- c5t Database Schema Migration: Add Projects with M:N Relationships
-- Adds project entity and converts direct foreign keys to join tables

-- ============================================================================
-- 1. CREATE PROJECT TABLE
-- ============================================================================

CREATE TABLE IF NOT EXISTS project (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    title TEXT NOT NULL,
    description TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- Auto-update timestamp trigger
CREATE TRIGGER IF NOT EXISTS project_update AFTER UPDATE ON project BEGIN
    UPDATE project SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- ============================================================================
-- 2. CREATE JOIN TABLES
-- ============================================================================

-- Project <-> Repo (M:N)
CREATE TABLE IF NOT EXISTS project_repo (
    project_id TEXT NOT NULL CHECK(length(project_id) == 8),
    repo_id TEXT NOT NULL CHECK(length(repo_id) == 8),
    created_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (project_id, repo_id),
    FOREIGN KEY (project_id) REFERENCES project(id) ON DELETE CASCADE,
    FOREIGN KEY (repo_id) REFERENCES repo(id) ON DELETE CASCADE
);

-- Project <-> Task List (M:N)
CREATE TABLE IF NOT EXISTS project_task_list (
    project_id TEXT NOT NULL CHECK(length(project_id) == 8),
    task_list_id TEXT NOT NULL CHECK(length(task_list_id) == 8),
    created_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (project_id, task_list_id),
    FOREIGN KEY (project_id) REFERENCES project(id) ON DELETE CASCADE,
    FOREIGN KEY (task_list_id) REFERENCES task_list(id) ON DELETE CASCADE
);

-- Project <-> Note (M:N)
CREATE TABLE IF NOT EXISTS project_note (
    project_id TEXT NOT NULL CHECK(length(project_id) == 8),
    note_id TEXT NOT NULL CHECK(length(note_id) == 8),
    created_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (project_id, note_id),
    FOREIGN KEY (project_id) REFERENCES project(id) ON DELETE CASCADE,
    FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE CASCADE
);

-- Task List <-> Repo (M:N) - replaces task_list.repo_id
CREATE TABLE IF NOT EXISTS task_list_repo (
    task_list_id TEXT NOT NULL CHECK(length(task_list_id) == 8),
    repo_id TEXT NOT NULL CHECK(length(repo_id) == 8),
    created_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (task_list_id, repo_id),
    FOREIGN KEY (task_list_id) REFERENCES task_list(id) ON DELETE CASCADE,
    FOREIGN KEY (repo_id) REFERENCES repo(id) ON DELETE CASCADE
);

-- Note <-> Repo (M:N) - replaces note.repo_id
CREATE TABLE IF NOT EXISTS note_repo (
    note_id TEXT NOT NULL CHECK(length(note_id) == 8),
    repo_id TEXT NOT NULL CHECK(length(repo_id) == 8),
    created_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (note_id, repo_id),
    FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE CASCADE,
    FOREIGN KEY (repo_id) REFERENCES repo(id) ON DELETE CASCADE
);

-- ============================================================================
-- 3. CREATE INDEXES FOR JOIN TABLES
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_project_repo_project ON project_repo(project_id);
CREATE INDEX IF NOT EXISTS idx_project_repo_repo ON project_repo(repo_id);

CREATE INDEX IF NOT EXISTS idx_project_task_list_project ON project_task_list(project_id);
CREATE INDEX IF NOT EXISTS idx_project_task_list_task_list ON project_task_list(task_list_id);

CREATE INDEX IF NOT EXISTS idx_project_note_project ON project_note(project_id);
CREATE INDEX IF NOT EXISTS idx_project_note_note ON project_note(note_id);

CREATE INDEX IF NOT EXISTS idx_task_list_repo_task_list ON task_list_repo(task_list_id);
CREATE INDEX IF NOT EXISTS idx_task_list_repo_repo ON task_list_repo(repo_id);

CREATE INDEX IF NOT EXISTS idx_note_repo_note ON note_repo(note_id);
CREATE INDEX IF NOT EXISTS idx_note_repo_repo ON note_repo(repo_id);

-- ============================================================================
-- 4. CREATE DEFAULT PROJECT AND MIGRATE DATA
-- ============================================================================

-- Insert Default project with random 8-char hex ID
INSERT INTO project (id, title, description)
VALUES (lower(hex(randomblob(4))), 'Default', 'Default project for migrated data');

-- Link all existing repos to Default project
INSERT INTO project_repo (project_id, repo_id)
SELECT p.id, r.id
FROM project p, repo r
WHERE p.title = 'Default';

-- Link all existing task_lists to Default project
INSERT INTO project_task_list (project_id, task_list_id)
SELECT p.id, tl.id
FROM project p, task_list tl
WHERE p.title = 'Default';

-- Link all existing notes to Default project
INSERT INTO project_note (project_id, note_id)
SELECT p.id, n.id
FROM project p, note n
WHERE p.title = 'Default';

-- Migrate task_list -> repo relationships to join table
INSERT INTO task_list_repo (task_list_id, repo_id)
SELECT id, repo_id
FROM task_list
WHERE repo_id IS NOT NULL;

-- Migrate note -> repo relationships to join table
INSERT INTO note_repo (note_id, repo_id)
SELECT id, repo_id
FROM note
WHERE repo_id IS NOT NULL;

-- ============================================================================
-- 5. REMOVE OLD FOREIGN KEY COLUMNS
-- ============================================================================

-- SQLite doesn't support DROP COLUMN directly (before 3.35.0)
-- We need to recreate the tables without the repo_id column

-- Recreate task_list without repo_id
CREATE TABLE task_list_new (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    name TEXT NOT NULL,
    description TEXT,
    notes TEXT,
    tags TEXT,
    external_ref TEXT,
    status TEXT DEFAULT 'active' CHECK(status IN ('active', 'archived')),
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),
    archived_at TEXT
);

INSERT INTO task_list_new (id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at)
SELECT id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at
FROM task_list;

DROP TABLE task_list;
ALTER TABLE task_list_new RENAME TO task_list;

-- Recreate indexes for task_list
CREATE INDEX IF NOT EXISTS idx_task_list_status ON task_list(status);

-- Recreate task_list_update trigger
CREATE TRIGGER IF NOT EXISTS task_list_update AFTER UPDATE ON task_list BEGIN
    UPDATE task_list SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- Recreate note without repo_id
CREATE TABLE note_new (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT,
    note_type TEXT DEFAULT 'manual' CHECK(note_type IN ('manual', 'archived_todo', 'scratchpad')),
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

INSERT INTO note_new (id, title, content, tags, note_type, created_at, updated_at)
SELECT id, title, content, tags, note_type, created_at, updated_at
FROM note;

DROP TABLE note;
ALTER TABLE note_new RENAME TO note;

-- Recreate indexes for note
CREATE INDEX IF NOT EXISTS idx_note_type ON note(note_type);

-- Recreate note_update trigger
CREATE TRIGGER IF NOT EXISTS note_update AFTER UPDATE ON note BEGIN
    UPDATE note SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- Recreate FTS triggers for note
CREATE TRIGGER IF NOT EXISTS note_ai AFTER INSERT ON note BEGIN
    INSERT INTO note_fts(rowid, title, content) 
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS note_au AFTER UPDATE ON note 
WHEN old.title != new.title OR old.content != new.content BEGIN
    INSERT INTO note_fts(note_fts, rowid, title, content) 
    VALUES('delete', old.rowid, old.title, old.content);
    INSERT INTO note_fts(rowid, title, content) 
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS note_ad AFTER DELETE ON note BEGIN
    INSERT INTO note_fts(note_fts, rowid, title, content) 
    VALUES('delete', old.rowid, old.title, old.content);
END;

-- Rebuild FTS index to sync with new rowids
INSERT INTO note_fts(note_fts) VALUES('rebuild');