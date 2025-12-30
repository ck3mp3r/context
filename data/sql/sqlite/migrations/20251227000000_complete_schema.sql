-- c5t Complete Database Schema
-- Single consolidated migration with all features
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
    name TEXT NOT NULL,
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
    content TEXT NOT NULL,
    status TEXT DEFAULT 'backlog' CHECK(status IN ('backlog', 'todo', 'in_progress', 'review', 'done', 'cancelled')),
    priority INTEGER CHECK(priority BETWEEN 1 AND 5 OR priority IS NULL),
    tags TEXT DEFAULT '[]',             -- JSON array
    created_at TEXT DEFAULT (datetime('now')),
    started_at TEXT,
    completed_at TEXT,
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
CREATE INDEX IF NOT EXISTS idx_task_list ON task(list_id);
CREATE INDEX IF NOT EXISTS idx_task_status ON task(status);
CREATE INDEX IF NOT EXISTS idx_task_list_status ON task(list_id, status);
CREATE INDEX IF NOT EXISTS idx_task_priority ON task(priority);
CREATE INDEX IF NOT EXISTS idx_task_list_priority ON task(list_id, priority);
CREATE INDEX IF NOT EXISTS idx_task_parent ON task(parent_id);
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

-- Full-text search virtual table for notes
CREATE VIRTUAL TABLE IF NOT EXISTS note_fts USING fts5(
    title,
    content,
    content='note',
    content_rowid='rowid'
);

-- FTS sync triggers - keep full-text index in sync with note table
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
-- DEFAULT DATA
-- ============================================================================

-- Insert Default project for general/uncategorized work
INSERT INTO project (id, title, description)
VALUES (lower(hex(randomblob(4))), 'Default', 'Default project for uncategorized work');
