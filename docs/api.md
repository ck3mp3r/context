# REST API

## Server

**Endpoint**: `http://localhost:3737`

**Documentation**: `/docs` (Scalar UI with OpenAPI spec, requires `--docs` flag)

## Endpoints

### System
- `GET /health` - Health check
- `ANY /ws` - WebSocket upgrade; streams change and update events to the client
- `ANY /mcp` - MCP server, JSON-RPC over Streamable HTTP

### Projects
- `GET /api/v1/projects` - List projects (`q=` for search, `tags=`, `limit=`, `offset=`, `sort=`, `order=`)
- `POST /api/v1/projects` - Create project
- `GET /api/v1/projects/{id}` - Get project
- `PUT /api/v1/projects/{id}` - Replace project
- `PATCH /api/v1/projects/{id}` - Partial update project
- `DELETE /api/v1/projects/{id}` - Soft-delete project
- `POST /api/v1/projects/{id}/restore` - Restore soft-deleted project

### Repositories
- `GET /api/v1/repos` - List repositories (`project_id=`, `tags=`, `limit=`, `offset=`, `sort=`, `order=`)
- `POST /api/v1/repos` - Create repository
- `GET /api/v1/repos/{id}` - Get repository
- `PUT /api/v1/repos/{id}` - Replace repository
- `PATCH /api/v1/repos/{id}` - Partial update repository
- `DELETE /api/v1/repos/{id}` - Soft-delete repository
- `POST /api/v1/repos/{id}/restore` - Restore soft-deleted repository

### Task Lists
- `GET /api/v1/task-lists` - List task lists (`q=` for search, `status=active|archived`, `tags=`, `project_id=`, `limit=`, `offset=`, `sort=`, `order=`, `include_notes=`)
- `POST /api/v1/task-lists` - Create task list
- `GET /api/v1/task-lists/{id}` - Get task list with relationships
- `PUT /api/v1/task-lists/{id}` - Replace task list
- `PATCH /api/v1/task-lists/{id}` - Partial update task list
- `DELETE /api/v1/task-lists/{id}` - Soft-delete task list
- `POST /api/v1/task-lists/{id}/restore` - Restore soft-deleted task list
- `GET /api/v1/task-lists/{id}/stats` - Get task counts per status for one task list

### Tasks
- `GET /api/v1/task-lists/{list_id}/tasks` - List tasks (`q=` for search, `status=backlog|todo|in_progress|review|done|cancelled`, `type=task|subtask`, `parent_id=`, `limit=`, `offset=`, `sort=`, `order=`, `include_description=`)
- `POST /api/v1/task-lists/{list_id}/tasks` - Create task
- `GET /api/v1/tasks/{id}` - Get task
- `PUT /api/v1/tasks/{id}` - Replace task
- `PATCH /api/v1/tasks/{id}` - Partial update task
- `DELETE /api/v1/tasks/{id}` - Soft-delete task
- `POST /api/v1/tasks/{id}/restore` - Restore soft-deleted task
- `GET /api/v1/tasks/{id}/transitions` - List status transitions (`limit=`, `offset=`)
- `POST /api/v1/tasks/{id}/transitions` - Transition task status; body `{"status": "<status>"}`

### Notes
- `GET /api/v1/notes` - List notes (`q=` for search, `type=note|subnote`, `status=active|archived`, `tags=`, `project_id=`, `parent_id=`, `limit=`, `offset=`, `sort=`, `order=`, `include_content=`)
- `POST /api/v1/notes` - Create note
- `GET /api/v1/notes/{id}` - Get note
- `PUT /api/v1/notes/{id}` - Replace note
- `PATCH /api/v1/notes/{id}` - Partial update note
- `DELETE /api/v1/notes/{id}` - Soft-delete note
- `POST /api/v1/notes/{id}/restore` - Restore soft-deleted note

### Skills
- `GET /api/v1/skills` - List skills (`q=` for search, `tags=`, `project_id=`, `limit=`, `offset=`, `sort=`, `order=`, `include_content=`)
- `POST /api/v1/skills` - Create skill
- `POST /api/v1/skills/import` - Import skill from a local path or a git repository
- `GET /api/v1/skills/{id}` - Get skill, with full content and frontmatter
- `PUT /api/v1/skills/{id}` - Replace skill
- `PATCH /api/v1/skills/{id}` - Partial update skill (tags, project_ids)
- `DELETE /api/v1/skills/{id}` - Soft-delete skill
- `POST /api/v1/skills/{id}/restore` - Restore soft-deleted skill
- `POST /api/v1/skills/{id}/refresh` - Re-read skill content from its source
- `POST /api/v1/skills/{id_or_name}/enable` - Enable skill for the agent
- `POST /api/v1/skills/{id_or_name}/disable` - Disable skill for the agent

### Sync
- `POST /api/v1/sync/init` - Initialize the git-backed sync repository
- `POST /api/v1/sync/export` - Export the database to the sync repository and commit
- `POST /api/v1/sync/import` - Import entities from the sync repository into the database
- `GET /api/v1/sync/status` - Get sync status
- `POST /api/v1/sync/enable` - Enable sync
- `POST /api/v1/sync/disable` - Disable sync
- `POST /api/v1/sync/remote` - Set the git remote URL

### Settings
- `GET /api/v1/settings` - Get stored values, built-in defaults, and the running configuration
- `PUT /api/v1/settings` - Store settings values

### Updater
- `GET /api/v1/update/status` - Get the persisted update state
- `POST /api/v1/update/check` - Start an update check; progress arrives over `/ws`
- `POST /api/v1/update/install` - Start an update install; progress arrives over `/ws`
- `POST /api/v1/update/restart` - Restart the server

## Running

```sh
# Development
cargo run --bin c5t -- api

# Production
cargo build --release
./target/release/c5t api

# With custom port
cargo run --bin c5t -- api --port 8080

# With custom skills directory (for OpenCode/Crush compatibility)
export C5T_SKILLS_DIR=~/.agents/skills
cargo run --bin c5t -- api

# Or use CLI flag (takes precedence over env var)
cargo run --bin c5t -- api --skills-dir ~/.agents/skills
```

## Configuration

**Port**: Default 3737 in release builds, 3738 in debug builds (override with `-p` or `--port`)

**Logging**: Default warn (increase with `-v`, `-vv`, or `-vvv`)

**Documentation**: Disabled by default (enable with `--docs`)

**Data Directory**: `~/.local/share/c5t` (override with `--home`)

**Skills Directory**: Default `~/.local/share/c5t/skills` (override with `--skills-dir` or `C5T_SKILLS_DIR` env var)
  - Controls where skill attachments are extracted when skills are loaded
  - Useful for sharing skills cache with other tools (e.g., `~/.agents/skills` for OpenCode/Crush compatibility)
  - Precedence: CLI flag > environment variable > default

## Example Requests

```sh
# Create project
curl -X POST http://localhost:3737/api/v1/projects \
  -H "Content-Type: application/json" \
  -d '{"title": "My Project", "description": "Project description"}'

# List active task lists
curl http://localhost:3737/api/v1/task-lists?status=active

# Search notes (q= parameter on the list route)
curl "http://localhost:3737/api/v1/notes?q=rust+async"

# Search skills (q= parameter on the list route)
curl "http://localhost:3737/api/v1/skills?q=kubernetes+deployment"

# Import skill from local directory
curl -X POST http://localhost:3737/api/v1/skills/import \
  -H "Content-Type: application/json" \
  -d '{
    "source": "./path/to/skill",
    "tags": ["deployment", "kubernetes"],
    "project_ids": ["abc12345"],
    "update": false
  }'

# Import skill from git repository
curl -X POST http://localhost:3737/api/v1/skills/import \
  -H "Content-Type: application/json" \
  -d '{
    "source": "git+https://github.com/user/skills-repo",
    "path": "deploy-k8s",
    "tags": ["deployment", "kubernetes"],
    "update": true
  }'

# Transition a task to done
curl -X POST http://localhost:3737/api/v1/tasks/abc12345/transitions \
  -H "Content-Type: application/json" \
  -d '{"status": "done"}'
```
