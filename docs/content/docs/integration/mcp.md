+++
title = "MCP"
description = "Connect an AI agent to c5t over MCP, and read the tool surface."
weight = 1
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

The c5t MCP server runs inside the API server, so one process serves the Web
UI, the REST API, and the MCP endpoint:

```bash
c5t api
```

| Surface | Address |
|---------|---------|
| MCP endpoint | `http://localhost:3737/mcp` |
| REST API | `http://localhost:3737/api/v1/*` |
| Web UI | `http://localhost:3737/` |
| WebSocket | `ws://localhost:3737/ws` |

The transport is Streamable HTTP with SSE. The client must support remote MCP
servers.

## Connect a client

The endpoint is a remote MCP server. An OpenCode entry in
`~/.config/opencode/opencode.json` looks like this:

```json
{
  "mcp": {
    "c5t": {
      "enabled": true,
      "type": "remote",
      "url": "http://localhost:3737/mcp"
    }
  }
}
```

Three rules:

- Start `c5t api` before the client connects. A client that starts first finds no
  endpoint and reports a failed handshake.
- Use `http://0.0.0.0:3737/mcp` when the client runs on another machine.
- Use port 3738 to reach a debug build. Port 3737 is the release build and holds
  your real data; port 3738 is the development database. A client that needs both
  keeps two entries with different names.

## Identifiers

Every entity carries an 8-character lowercase hex ID, such as `a1b2c3d4`. Read
an ID from the Web UI copy icon, from `--json` output on the CLI, or from the
`id` field of any MCP response.

## Tools

The server exposes 37 tools in seven groups.

### Projects (6)

- `project_list` — list projects with pagination and sorting
- `project_get` — get a project by ID
- `project_create` — create a project
- `project_update` — update a project
- `project_delete` — delete a project (soft delete)
- `project_restore` — restore a soft-deleted project

### Repositories (6)

- `repo_list` — list repositories with pagination and sorting
- `repo_get` — get a repository by ID
- `repo_create` — register a git repository
- `repo_update` — update a repository
- `repo_delete` — delete a repository (soft delete)
- `repo_restore` — restore a soft-deleted repository

### Task lists (7)

- `task_list_list` — list task lists (filter by status, tags, project)
- `task_list_get` — get a task list with its relationships
- `task_list_create` — create a task list
- `task_list_update` — update a task list
- `task_list_delete` — delete a task list (soft delete)
- `task_list_restore` — restore a soft-deleted task list
- `task_list_stats_get` — task counts by status for one task list

### Tasks (7)

- `task_list` — list tasks (filter by status, parent)
- `task_get` — get a task by ID
- `task_create` — create a task; it starts in `backlog`
- `task_update` — update title, description, priority, tags, and relationships
- `task_transition` — transition one or more tasks to a new status
- `task_delete` — delete a task (soft delete)
- `task_restore` — restore a soft-deleted task

### Notes (6)

- `note_list` — list notes (filter by tags, type, and an optional FTS5 query)
- `note_create` — create a note
- `note_read` — read note content as TOON or JSON, optionally by line range
- `note_edit` — update metadata, or patch the content by line range
- `note_delete` — delete a note (soft delete)
- `note_restore` — restore a soft-deleted note

### Skills (4)

- `skill_list` — list and search skills with FTS5
- `skill_get` — get a skill by ID together with its attachment cache
- `skill_update` — replace the tags and project links of a skill
- `skill_restore` — restore a soft-deleted skill

### Sync (1)

- `sync` — `init`, `export`, `import`, and `status`

## Tags

Tags organise rows, filter queries, and link related notes.

A descriptive tag is lower case: `session`, `kubernetes`, `tdd`, `bugfix`.

A reference tag links one note to another, in the form `type:note_id`:

| Tag | Meaning |
|-----|---------|
| `parent:NOTE_ID` | this note continues that note |
| `related:NOTE_ID` | see also that note |
| `supersedes:NOTE_ID` | this note replaces that note |
| `obsoletes:NOTE_ID` | this note makes that note obsolete |

```javascript
// Notes that continue note abc123ef
note_list({ tags: ["parent:abc123ef"] })

// Notes that reference it
note_list({ tags: ["related:abc123ef"] })
```

Reference tags use the existing tag column, so they need no schema change and
they are already indexed.

## Note size limits

A note that is too large overflows the agent's context window. Three limits
apply:

| Limit | Size | Behaviour |
|-------|------|-----------|
| Warning | 10,000 characters | the write succeeds and returns a warning |
| Soft maximum | 50,000 characters | split the note before this size |
| Hard maximum | 100,000 characters | the write fails |

Keep a note under 10,000 characters. To hold more, split the text across notes
and chain them with `parent:NOTE_ID`. `note_list` returns metadata only, so use
`note_read` to fetch content.

## Skills

A skill holds a complete `SKILL.md`: YAML frontmatter plus a Markdown body.
`skill_get` returns the whole file in the `content` field, and the agent parses
the frontmatter itself. `name` and `description` are required in the frontmatter;
the database indexes both of them.

```javascript
skill_list({ query: "kubernetes AND deployment" })
skill_get({ skill_id: "skill123" })
```

`skill_update` replaces the whole tag list and the whole project link list. Read
the skill first when you want to keep the current values.

## What's next?

- [REST API](@/docs/integration/rest-api.md) — the same operations over HTTP.
- [Sync](@/docs/integration/sync.md) — move the store between machines.
- [CLI](@/docs/usage/cli.md) — the same operations from a terminal.
