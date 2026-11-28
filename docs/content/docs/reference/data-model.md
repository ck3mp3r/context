+++
title = "Data model"
description = "The tables, the identifiers, the tombstones, and the search indexes."
weight = 1
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

c5t stores everything in one SQLite database. Eight tables hold the rows, five
join tables link them, and six FTS5 tables make them searchable.

Every ID is an 8-character lower-case hex string, and each table enforces it:

```sql
id TEXT PRIMARY KEY CHECK(length(id) == 8)
```

Timestamps are ISO 8601 text in the form `YYYY-MM-DD HH:MM:SS`. Tags are JSON
arrays in a text column, such as `["rust", "async"]`.

## Entities

### project

| Column | Type | Meaning |
|--------|------|---------|
| `id` | TEXT | primary key |
| `title` | TEXT | required |
| `description` | TEXT | optional |
| `tags` | TEXT | JSON array |
| `external_refs` | TEXT | JSON array, required, default `[]` |
| `created_at`, `updated_at` | TEXT | timestamps |
| `deleted_at` | TEXT | NULL while live |

### repo

| Column | Type | Meaning |
|--------|------|---------|
| `id` | TEXT | primary key |
| `remote` | TEXT | git remote, unique, required |
| `path` | TEXT | last known local path |
| `tags` | TEXT | JSON array |
| `created_at`, `updated_at` | TEXT | timestamps |
| `deleted_at` | TEXT | NULL while live |

### task_list

| Column | Type | Meaning |
|--------|------|---------|
| `id` | TEXT | primary key |
| `title` | TEXT | required |
| `description` | TEXT | optional |
| `notes` | TEXT | Markdown progress notes |
| `tags` | TEXT | JSON array |
| `external_refs` | TEXT | JSON array, default `[]` |
| `status` | TEXT | `active` or `archived` |
| `project_id` | TEXT | foreign key to `project`, required |
| `created_at`, `updated_at` | TEXT | timestamps |
| `archived_at` | TEXT | set when archived |
| `deleted_at` | TEXT | NULL while live |

A task list belongs to exactly one project. The foreign key cascades on delete.

### task

| Column | Type | Meaning |
|--------|------|---------|
| `id` | TEXT | primary key |
| `list_id` | TEXT | foreign key to `task_list`, required |
| `parent_id` | TEXT | foreign key to `task`, NULL for a root task |
| `title` | TEXT | required, at most 500 characters |
| `description` | TEXT | optional, at most 10,000 characters |
| `status` | TEXT | one of six states |
| `priority` | INTEGER | 1 to 5, where 1 is highest |
| `tags` | TEXT | JSON array, required |
| `external_refs` | TEXT | JSON array, default `[]` |
| `created_at`, `updated_at` | TEXT | timestamps, required |
| `deleted_at` | TEXT | NULL while live |

The status check accepts `backlog`, `todo`, `in_progress`, `review`, `done`, and
`cancelled`. `parent_id` cascades, so deleting a parent task removes its
subtasks.

### task_transition_log

| Column | Type | Meaning |
|--------|------|---------|
| `id` | TEXT | primary key |
| `task_id` | TEXT | foreign key to `task`, required |
| `status` | TEXT | the status that was entered |
| `transitioned_at` | TEXT | timestamp, default `now` |

Every status change adds a row. The `started_at` and `completed_at` columns are
gone; the log replaces them. This is the only syncable table without
`updated_at`, so an import upserts its values directly.

### note

| Column | Type | Meaning |
|--------|------|---------|
| `id` | TEXT | primary key |
| `title` | TEXT | required |
| `content` | TEXT | Markdown, required |
| `tags` | TEXT | JSON array |
| `parent_id` | TEXT | foreign key to `note`, NULL for a root note |
| `idx` | INTEGER | manual order among siblings, lower first |
| `status` | TEXT | `active` or `archived` |
| `created_at`, `updated_at` | TEXT | timestamps |
| `archived_at` | TEXT | set when archived |
| `deleted_at` | TEXT | NULL while live |

Archive is a live state: an archived note is still exported and still
searchable. It is independent of `deleted_at`.

### skill

| Column | Type | Meaning |
|--------|------|---------|
| `id` | TEXT | primary key |
| `name` | TEXT | required, from the frontmatter |
| `description` | TEXT | required, from the frontmatter |
| `content` | TEXT | the whole `SKILL.md`: frontmatter plus body |
| `tags` | TEXT | JSON array |
| `source`, `source_subpath` | TEXT | where an import came from; NULL when unknown |
| `created_at`, `updated_at` | TEXT | timestamps |
| `deleted_at` | TEXT | NULL while live |

The `content` column holds the file exactly as written. `name` and `description`
are copies extracted for indexing and search. An agent parses the frontmatter
from `content`, so a new frontmatter field needs no schema change.

### skill_attachment

| Column | Type | Meaning |
|--------|------|---------|
| `id` | TEXT | primary key |
| `skill_id` | TEXT | foreign key to `skill`, required |
| `type` | TEXT | `script`, `reference`, or `asset` |
| `filename` | TEXT | relative file name |
| `content` | TEXT | base64-encoded file bytes |
| `content_hash` | TEXT | SHA-256 of the content, for cache invalidation |
| `mime_type` | TEXT | optional |
| `created_at`, `updated_at` | TEXT | timestamps |
| `deleted_at` | TEXT | NULL while live |

`UNIQUE(skill_id, type, filename)` keeps one file per type and name. Attachments
are extracted to the skills cache by `skill enable`.

## Relationship tables

Each join table holds a pair of IDs as its primary key:

| Table | Links |
|-------|-------|
| `project_repo` | project to repo |
| `project_note` | project to note |
| `task_list_repo` | task list to repo |
| `note_repo` | note to repo |
| `project_skill` | project to skill |

Every foreign key in these tables cascades on delete.

## Tombstones

c5t does not hard-delete an entity during normal operation. Each entity table
carries `deleted_at`:

- `NULL` means the row is live.
- A timestamp marks a tombstone. Reads and searches skip it; sync keeps it.

Delete sets the column and leaves the row in place. Restore clears the column.
The tombstone is what carries the deletion to another machine; see the
[sync page](@/docs/integration/sync.md) for how tombstones propagate and when
they are purged.

## Search

Six FTS5 virtual tables index the rows:

| Table | Indexed fields |
|-------|----------------|
| `project_fts` | title, description, tags, external refs |
| `task_list_fts` | title, description, notes, tags, external refs |
| `task_fts` | title, description, tags, external refs |
| `repo_fts` | remote, path, tags |
| `note_fts` | title, content, tags |
| `skill_fts` | name, description, content, tags |

Triggers keep each index in step with its table. The triggers index a row only
while `deleted_at IS NULL` and remove the entry when a row is soft-deleted, so a
search never returns a tombstone. Restoring a note re-inserts its entry.

`note_fts` is an external-content table over `note`, so it uses the FTS5
`delete` command instead of a direct row delete.

## Sync bookkeeping

`sync_state` holds exactly one row, with `id = 1`:

| Column | Meaning |
|--------|---------|
| `last_synced_at` | cutoff of the last successful export |
| `last_full_sync_at` | when the last full export ran |
| `schema_version` | schema version that produced the cutoff |

A schema change resets `last_synced_at` to NULL, so the next export reports
every row as changed.

## Migrations

The SQL lives in `crates/context-db/data/sql/sqlite/migrations/`. SQLx applies
the pending files on startup and records them in the `_sqlx_migrations` table.

The schema version is the highest `YYYYMMDDHHMMSS` prefix in that directory.

## What's next?

- [Sync](@/docs/integration/sync.md) — how the tables travel between machines.
- [REST API](@/docs/integration/rest-api.md) — the JSON shape of these rows.
- [MCP](@/docs/integration/mcp.md) — the tool surface over the same rows.
