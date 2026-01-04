# Database Schema

## Overview

SQLite database with M:N relationship tables for flexible organization.

## Core Tables

### project
```sql
id          TEXT PRIMARY KEY    -- 8-char hex
title       TEXT NOT NULL
description TEXT
tags        TEXT                -- JSON array
created_at  TEXT
updated_at  TEXT
```

### repo
```sql
id         TEXT PRIMARY KEY    -- 8-char hex
remote     TEXT NOT NULL       -- Git remote URL
path       TEXT                -- Local filesystem path
tags       TEXT                -- JSON array
created_at TEXT
```

### task_list
```sql
id           TEXT PRIMARY KEY    -- 8-char hex
name         TEXT NOT NULL
description  TEXT
notes        TEXT                -- Markdown progress notes
tags         TEXT                -- JSON array
external_ref TEXT                -- e.g., "JIRA-123"
status       TEXT                -- 'active' | 'archived'
project_id   TEXT NOT NULL       -- FK to project
created_at   TEXT
updated_at   TEXT
archived_at  TEXT
```

### task
```sql
id           TEXT PRIMARY KEY    -- 8-char hex
list_id      TEXT NOT NULL       -- FK to task_list
parent_id    TEXT                -- FK to task (subtasks)
content      TEXT NOT NULL
status       TEXT                -- 'backlog' | 'todo' | 'in_progress' | 'review' | 'done' | 'cancelled'
priority     INTEGER             -- 1-5 (1=highest)
tags         TEXT                -- JSON array
external_ref TEXT                -- e.g., "owner/repo#123", "https://jira.example.com/browse/PROJ-123"
created_at   TEXT
started_at   TEXT
completed_at TEXT
updated_at   TEXT
```

### note
```sql
id         TEXT PRIMARY KEY    -- 8-char hex
title      TEXT NOT NULL
content    TEXT NOT NULL       -- Markdown
tags       TEXT                -- JSON array
note_type  TEXT                -- 'manual' | 'archived_todo'
created_at TEXT
updated_at TEXT
```

## Relationship Tables

### project_repo
```sql
project_id TEXT NOT NULL    -- FK to project
repo_id    TEXT NOT NULL    -- FK to repo
PRIMARY KEY (project_id, repo_id)
```

### project_note
```sql
project_id TEXT NOT NULL    -- FK to project
note_id    TEXT NOT NULL    -- FK to note
PRIMARY KEY (project_id, note_id)
```

### task_list_repo
```sql
task_list_id TEXT NOT NULL    -- FK to task_list
repo_id      TEXT NOT NULL    -- FK to repo
PRIMARY KEY (task_list_id, repo_id)
```

### note_repo
```sql
note_id TEXT NOT NULL    -- FK to note
repo_id TEXT NOT NULL    -- FK to repo
PRIMARY KEY (note_id, repo_id)
```

## Full-Text Search

### note_fts
```sql
-- FTS5 virtual table for note search
-- Indexes: title, content, tags
-- Triggered on INSERT/UPDATE/DELETE
```

## Constraints

- All IDs are 8-character lowercase hex strings
- CHECK constraints enforce valid ID format
- Timestamps are ISO8601 TEXT: `YYYY-MM-DD HH:MM:SS`
- Tags stored as JSON arrays: `["tag1", "tag2"]`
- CASCADE DELETE on repo foreign keys

## Migrations

Location: `data/sql/sqlite/migrations/`

Applied automatically by SQLx on startup.

Current schema version: `20251227000000_complete_schema.sql`
