+++
title = "REST API"
description = "The HTTP surface under /api/v1: endpoints, parameters, and example requests."
weight = 2
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

The API server answers on `http://localhost:3737`. The OpenAPI description at
`/docs` is off by default; start the server with `--docs` to serve it:

```sh
c5t api --docs
```

A debug build binds 3738 and uses the development database. A release build
binds 3737.

## System

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/health` | health check, returns `{"status":"ok"}` |
| `GET` | `/ws` | WebSocket for live updates |

## Projects

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/projects` | list projects |
| `POST` | `/api/v1/projects` | create a project (201) |
| `GET` | `/api/v1/projects/:id` | get a project |
| `PUT` | `/api/v1/projects/:id` | replace a project |
| `PATCH` | `/api/v1/projects/:id` | update named fields |
| `DELETE` | `/api/v1/projects/:id` | soft delete a project |
| `POST` | `/api/v1/projects/:id/restore` | restore a deleted project |

## Repositories

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/repos` | list repositories |
| `POST` | `/api/v1/repos` | register a repository |
| `GET` | `/api/v1/repos/:id` | get a repository |
| `PUT` | `/api/v1/repos/:id` | replace a repository |
| `PATCH` | `/api/v1/repos/:id` | update named fields |
| `DELETE` | `/api/v1/repos/:id` | soft delete a repository |
| `POST` | `/api/v1/repos/:id/restore` | restore a deleted repository |

## Task lists

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/task-lists` | list task lists (filter by status, tags, project) |
| `POST` | `/api/v1/task-lists` | create a task list |
| `GET` | `/api/v1/task-lists/:id` | get a task list with its relationships |
| `PUT` | `/api/v1/task-lists/:id` | replace a task list |
| `PATCH` | `/api/v1/task-lists/:id` | update named fields |
| `DELETE` | `/api/v1/task-lists/:id` | soft delete a task list |
| `POST` | `/api/v1/task-lists/:id/restore` | restore a deleted task list |
| `GET` | `/api/v1/task-lists/:id/stats` | task counts by status |

## Tasks

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/task-lists/:list_id/tasks` | list the tasks of one list |
| `POST` | `/api/v1/task-lists/:list_id/tasks` | create a task in that list |
| `GET` | `/api/v1/tasks/:id` | get a task |
| `PUT` | `/api/v1/tasks/:id` | replace a task |
| `PATCH` | `/api/v1/tasks/:id` | update named fields |
| `DELETE` | `/api/v1/tasks/:id` | soft delete a task |
| `POST` | `/api/v1/tasks/:id/restore` | restore a deleted task |
| `GET` | `/api/v1/tasks/:id/transitions` | read the transition log |
| `POST` | `/api/v1/tasks/:id/transitions` | transition the task status |

The transition body names the target status:

```sh
curl -X POST http://localhost:3737/api/v1/tasks/abc12345/transitions \
  -H "Content-Type: application/json" \
  -d '{"status": "in_progress"}'
```

The status is one of `backlog`, `todo`, `in_progress`, `review`, `done`, or
`cancelled`. Two guards apply: `review` requires `in_progress`, and `done`
requires `review`.

## Notes

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/notes` | list notes (filter by tags, type, status, project; FTS5 via `q`) |
| `POST` | `/api/v1/notes` | create a note |
| `GET` | `/api/v1/notes/:id` | get a note |
| `PUT` | `/api/v1/notes/:id` | replace a note |
| `PATCH` | `/api/v1/notes/:id` | update named fields |
| `DELETE` | `/api/v1/notes/:id` | soft delete a note |
| `POST` | `/api/v1/notes/:id/restore` | restore a deleted note |

Full-text search runs through the list endpoint. `include_content` defaults to
`false`, so add it when you need the bodies:

```sh
curl "http://localhost:3737/api/v1/notes?q=rust+AND+async&include_content=true"
```

## Skills

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/skills` | list skills (filter by tags, project; FTS5 via `q`) |
| `POST` | `/api/v1/skills` | create a skill |
| `POST` | `/api/v1/skills/import` | import a skill from a path, git repo, or archive URL |
| `GET` | `/api/v1/skills/:id` | get a skill with its full `SKILL.md` |
| `PUT` | `/api/v1/skills/:id` | replace a skill |
| `PATCH` | `/api/v1/skills/:id` | update tags and project links |
| `POST` | `/api/v1/skills/:id_or_name/enable` | extract the attachments to the cache |
| `POST` | `/api/v1/skills/:id_or_name/disable` | remove the cached files |
| `POST` | `/api/v1/skills/:id/refresh` | re-read the skill from its source |
| `DELETE` | `/api/v1/skills/:id` | soft delete a skill |
| `POST` | `/api/v1/skills/:id/restore` | restore a deleted skill |

```sh
# Import a skill from a local directory
curl -X POST http://localhost:3737/api/v1/skills/import \
  -H "Content-Type: application/json" \
  -d '{
    "source": "./path/to/skill",
    "tags": ["deployment", "kubernetes"],
    "project_ids": ["abc12345"],
    "update": false
  }'

# Import a skill from a git repository
curl -X POST http://localhost:3737/api/v1/skills/import \
  -H "Content-Type: application/json" \
  -d '{
    "source": "git+https://github.com/user/skills-repo",
    "path": "deploy-k8s",
    "tags": ["deployment", "kubernetes"],
    "update": true
  }'
```

## Sync

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/sync/status` | initialization, remote, counts |
| `POST` | `/api/v1/sync/init` | initialize the sync repository |
| `POST` | `/api/v1/sync/export` | export the database to JSONL and commit |
| `POST` | `/api/v1/sync/import` | import the JSONL files into the database |
| `POST` | `/api/v1/sync/enable` | re-enable sync |
| `POST` | `/api/v1/sync/disable` | disable sync |
| `POST` | `/api/v1/sync/remote` | set the git remote |

## Settings and updates

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/settings` | stored settings, defaults, and running values |
| `PUT` | `/api/v1/settings` | write stored settings |
| `GET` | `/api/v1/update/status` | updater state |
| `POST` | `/api/v1/update/check` | check for a release, body `{"manual": true}` |
| `POST` | `/api/v1/update/install` | download, verify, and stage the release |
| `POST` | `/api/v1/update/restart` | restart into the staged release |

## List parameters

Every list endpoint takes the same paging parameters:

| Parameter | Meaning |
|-----------|---------|
| `limit` | maximum number of items |
| `offset` | number of items to skip |
| `sort` | field to sort by |
| `order` | `asc` or `desc` |
| `tags` | comma-separated tags; a row matches any of them |
| `q` | FTS5 query where the endpoint supports search |

A list response carries the page and the total:

```json
{
  "items": [],
  "total": 0,
  "limit": 10,
  "offset": 0
}
```

## Example requests

```sh
# Create a project
curl -X POST http://localhost:3737/api/v1/projects \
  -H "Content-Type: application/json" \
  -d '{"title": "My Project", "description": "Project description"}'

# List active task lists
curl http://localhost:3737/api/v1/task-lists?status=active

# Search notes
curl "http://localhost:3737/api/v1/notes?q=rust+async"

# Search skills
curl "http://localhost:3737/api/v1/skills?q=kubernetes+deployment"

# Read the task counts of one task list
curl http://localhost:3737/api/v1/task-lists/abc12345/stats
```

An error response carries a single `error` string:

```json
{
  "error": "Entity not found: project with id 'zzzzzzzz'"
}
```

## What's next?

- [MCP](@/docs/integration/mcp.md) — the same store as MCP tools.
- [Sync](@/docs/integration/sync.md) — the git-backed export and import.
- [Data model](@/docs/reference/data-model.md) — the rows these endpoints return.
