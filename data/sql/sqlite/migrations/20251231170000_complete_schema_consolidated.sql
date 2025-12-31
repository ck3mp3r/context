-- c5t Complete Database Schema - CONSOLIDATED MIGRATION
-- This migration replaces all previous migrations
-- SQLite database for context/memory management with project support
-- Uses 8-character hex TEXT primary keys for sync support

-- ============================================================================
-- CORE TABLES
-- ============================================================================

-- Repository table - tracks git repositories
CREATE TABLE IF NOT EXISTS repo (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    remote TEXT UNIQUE NOT NULL,        -- e.g. "github:ck3mp3r/nu-mcp"
    path TEXT,                          -- local absolute path (last known)
    tags TEXT DEFAULT '[]',             -- JSON array
    created_at TEXT DEFAULT (datetime('now'))
);

-- Project table - groups related repos, task lists, and notes
CREATE TABLE IF NOT EXISTS project (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    title TEXT NOT NULL,
    description TEXT,
    tags TEXT DEFAULT '[]',             -- JSON array
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- Task List table - collections of work items (belongs to ONE project)
CREATE TABLE IF NOT EXISTS task_list (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    title TEXT NOT NULL,                -- NOTE: renamed from 'name' for consistency
    description TEXT,
    notes TEXT,
    tags TEXT DEFAULT '[]',             -- JSON array
    external_ref TEXT,                  -- Optional external reference (Jira ticket, GitHub issue, etc.)
    status TEXT DEFAULT 'active' CHECK(status IN ('active', 'archived')),
    project_id TEXT NOT NULL CHECK(length(project_id) == 8),  -- 1:N - task list belongs to ONE project
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),
    archived_at TEXT,
    FOREIGN KEY (project_id) REFERENCES project(id) ON DELETE CASCADE
);

-- Task table - individual work items within lists (supports subtasks via parent_id)
CREATE TABLE IF NOT EXISTS task (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    list_id TEXT NOT NULL CHECK(length(list_id) == 8),
    parent_id TEXT CHECK(parent_id IS NULL OR length(parent_id) == 8),  -- NULL = root task, otherwise FK to task.id
    title TEXT NOT NULL CHECK(length(title) <= 500),           -- NOTE: renamed from 'content'
    description TEXT CHECK(description IS NULL OR length(description) <= 10000),  -- NOTE: added for consistency
    status TEXT NOT NULL CHECK(status IN ('backlog', 'todo', 'in_progress', 'review', 'done', 'cancelled')),
    priority INTEGER CHECK(priority BETWEEN 1 AND 5),
    tags TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags)),   -- JSON array
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    updated_at TEXT NOT NULL,                                  -- NOTE: added for parent-child sync
    FOREIGN KEY (list_id) REFERENCES task_list(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_id) REFERENCES task(id) ON DELETE CASCADE
);

-- Note table - persistent markdown notes
CREATE TABLE IF NOT EXISTS note (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT DEFAULT '[]',             -- JSON array
    note_type TEXT DEFAULT 'manual' CHECK(note_type IN ('manual', 'archived_todo', 'scratchpad')),
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- ============================================================================
-- M:N RELATIONSHIP TABLES (JOIN TABLES)
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

-- Project <-> Note (M:N)
CREATE TABLE IF NOT EXISTS project_note (
    project_id TEXT NOT NULL CHECK(length(project_id) == 8),
    note_id TEXT NOT NULL CHECK(length(note_id) == 8),
    created_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (project_id, note_id),
    FOREIGN KEY (project_id) REFERENCES project(id) ON DELETE CASCADE,
    FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE CASCADE
);

-- Task List <-> Repo (M:N)
CREATE TABLE IF NOT EXISTS task_list_repo (
    task_list_id TEXT NOT NULL CHECK(length(task_list_id) == 8),
    repo_id TEXT NOT NULL CHECK(length(repo_id) == 8),
    created_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (task_list_id, repo_id),
    FOREIGN KEY (task_list_id) REFERENCES task_list(id) ON DELETE CASCADE,
    FOREIGN KEY (repo_id) REFERENCES repo(id) ON DELETE CASCADE
);

-- Note <-> Repo (M:N)
CREATE TABLE IF NOT EXISTS note_repo (
    note_id TEXT NOT NULL CHECK(length(note_id) == 8),
    repo_id TEXT NOT NULL CHECK(length(repo_id) == 8),
    created_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (note_id, repo_id),
    FOREIGN KEY (note_id) REFERENCES note(id) ON DELETE CASCADE,
    FOREIGN KEY (repo_id) REFERENCES repo(id) ON DELETE CASCADE
);

-- ============================================================================
-- INDEXES
-- ============================================================================

-- Core table indexes
CREATE INDEX IF NOT EXISTS idx_repo_remote ON repo(remote);
CREATE INDEX IF NOT EXISTS idx_task_list_status ON task_list(status);
CREATE INDEX IF NOT EXISTS idx_task_list_project ON task_list(project_id);
CREATE INDEX IF NOT EXISTS idx_task_list_id ON task(list_id);
CREATE INDEX IF NOT EXISTS idx_task_status ON task(status);
CREATE INDEX IF NOT EXISTS idx_task_list_status ON task(list_id, status);
CREATE INDEX IF NOT EXISTS idx_task_priority ON task(priority);
CREATE INDEX IF NOT EXISTS idx_task_list_priority ON task(list_id, priority);
CREATE INDEX IF NOT EXISTS idx_task_parent_id ON task(parent_id);
CREATE INDEX IF NOT EXISTS idx_task_created_at ON task(created_at);
CREATE INDEX IF NOT EXISTS idx_task_updated_at ON task(updated_at);
CREATE INDEX IF NOT EXISTS idx_note_type ON note(note_type);

-- Join table indexes
CREATE INDEX IF NOT EXISTS idx_project_repo_project ON project_repo(project_id);
CREATE INDEX IF NOT EXISTS idx_project_repo_repo ON project_repo(repo_id);
CREATE INDEX IF NOT EXISTS idx_project_note_project ON project_note(project_id);
CREATE INDEX IF NOT EXISTS idx_project_note_note ON project_note(note_id);
CREATE INDEX IF NOT EXISTS idx_task_list_repo_task_list ON task_list_repo(task_list_id);
CREATE INDEX IF NOT EXISTS idx_task_list_repo_repo ON task_list_repo(repo_id);
CREATE INDEX IF NOT EXISTS idx_note_repo_note ON note_repo(note_id);
CREATE INDEX IF NOT EXISTS idx_note_repo_repo ON note_repo(repo_id);

-- ============================================================================
-- FULL-TEXT SEARCH
-- ============================================================================

-- Full-text search virtual table for notes (includes tags)
CREATE VIRTUAL TABLE IF NOT EXISTS note_fts USING fts5(
    title,
    content,
    tags,
    content='note',
    content_rowid='rowid'
);

-- FTS sync triggers for notes - keep full-text index in sync with note table
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

-- Full-text search virtual table for tasks
CREATE VIRTUAL TABLE IF NOT EXISTS task_fts USING fts5(
    id UNINDEXED,
    title,
    description,
    tags
);

-- FTS sync triggers for tasks
CREATE TRIGGER IF NOT EXISTS task_fts_insert AFTER INSERT ON task BEGIN
    INSERT INTO task_fts(id, title, description, tags)
    VALUES (new.id, new.title, new.description, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS task_fts_delete AFTER DELETE ON task BEGIN
    DELETE FROM task_fts WHERE id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS task_fts_update AFTER UPDATE ON task BEGIN
    DELETE FROM task_fts WHERE id = old.id;
    INSERT INTO task_fts(id, title, description, tags)
    VALUES (new.id, new.title, new.description, new.tags);
END;

-- ============================================================================
-- AUTO-UPDATE TRIGGERS
-- ============================================================================

CREATE TRIGGER IF NOT EXISTS project_update AFTER UPDATE ON project BEGIN
    UPDATE project SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS task_list_update AFTER UPDATE ON task_list BEGIN
    UPDATE task_list SET updated_at = datetime('now') WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS note_update AFTER UPDATE ON note BEGIN
    UPDATE note SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- ============================================================================
-- TASK CASCADE TRIGGERS (PARENT-CHILD SYNC)
-- ============================================================================

-- Cascade updated_at to parent when subtask is UPDATED
-- CRITICAL: Uses child's updated_at (new.updated_at) NOT datetime('now')
-- This preserves historical timestamps during import
CREATE TRIGGER task_cascade_updated_at_to_parent AFTER UPDATE ON task
WHEN new.parent_id IS NOT NULL
BEGIN
    UPDATE task 
    SET updated_at = new.updated_at
    WHERE id = new.parent_id;
END;

-- Cascade updated_at to parent when subtask is INSERTED
-- CRITICAL: Uses child's updated_at (new.updated_at) NOT datetime('now')
-- This preserves historical timestamps during import
CREATE TRIGGER task_cascade_updated_at_on_insert AFTER INSERT ON task
WHEN new.parent_id IS NOT NULL
BEGIN
    UPDATE task 
    SET updated_at = new.updated_at
    WHERE id = new.parent_id;
END;

-- ============================================================================
-- DEFAULT DATA
-- ============================================================================

-- Insert Default project for general/uncategorized work
INSERT INTO project (id, title, description)
VALUES (lower(hex(randomblob(4))), 'Default', 'Default project for uncategorized work');
