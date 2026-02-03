# REST API

## Server

**Endpoint**: `http://localhost:3737`

**Documentation**: `/docs` (Scalar UI with OpenAPI spec, requires `--docs` flag)

## Endpoints

### System
- `GET /health` - Health check

### Projects
- `GET /api/v1/projects` - List all projects
- `POST /api/v1/projects` - Create project
- `GET /api/v1/projects/:id` - Get project
- `PUT /api/v1/projects/:id` - Update project
- `DELETE /api/v1/projects/:id` - Delete project

### Repositories
- `GET /api/v1/repos` - List all repositories
- `POST /api/v1/repos` - Create repository
- `GET /api/v1/repos/:id` - Get repository
- `PUT /api/v1/repos/:id` - Update repository
- `DELETE /api/v1/repos/:id` - Delete repository

### Task Lists
- `GET /api/v1/task-lists` - List task lists (filter by status, tags, project)
- `POST /api/v1/task-lists` - Create task list
- `GET /api/v1/task-lists/:id` - Get task list with relationships
- `PUT /api/v1/task-lists/:id` - Update task list
- `DELETE /api/v1/task-lists/:id` - Delete task list

### Tasks
- `GET /api/v1/task-lists/:list_id/tasks` - List tasks (filter by status, parent)
- `POST /api/v1/task-lists/:list_id/tasks` - Create task
- `GET /api/v1/tasks/:id` - Get task
- `PUT /api/v1/tasks/:id` - Update task
- `PATCH /api/v1/tasks/:id/transition` - Transition task status
- `DELETE /api/v1/tasks/:id` - Delete task

### Notes
- `GET /api/v1/notes` - List notes (filter by tags, note_type)
- `POST /api/v1/notes` - Create note
- `GET /api/v1/notes/:id` - Get note
- `PUT /api/v1/notes/:id` - Update note
- `DELETE /api/v1/notes/:id` - Delete note
- `GET /api/v1/notes/search?q=query` - Full-text search

### Skills
- `GET /api/v1/skills` - List skills (filter by tags, project_id)
- `POST /api/v1/skills/import` - Import skill from source (local path or git repo)
- `GET /api/v1/skills/:id` - Get skill (returns full content with frontmatter)
- `PATCH /api/v1/skills/:id` - Partial update skill (update tags, project_ids, etc.)
- `DELETE /api/v1/skills/:id` - Delete skill
- `GET /api/v1/skills/search?q=query` - Full-text search (searches name, description, content, tags)

## Running

```sh
# Development
cargo run --bin c5t -- api

# Production
cargo build --release
./target/release/c5t api

# With custom port
cargo run --bin c5t -- api --port 8080
```

## Configuration

**Port**: Default 3737 (override with `-p` or `--port`)

**Logging**: Default warn (increase with `-v`, `-vv`, or `-vvv`)

**Documentation**: Disabled by default (enable with `--docs`)

**Data Directory**: `~/.local/share/c5t` (override with `--home`)

## Example Requests

```sh
# Create project
curl -X POST http://localhost:3737/api/v1/projects \
  -H "Content-Type: application/json" \
  -d '{"title": "My Project", "description": "Project description"}'

# List active task lists
curl http://localhost:3737/api/v1/task-lists?status=active

# Search notes
curl "http://localhost:3737/api/v1/notes/search?q=rust+async"

# Search skills
curl "http://localhost:3737/api/v1/skills/search?q=kubernetes+deployment"

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

# Complete a task
curl -X PATCH http://localhost:3737/api/v1/tasks/abc12345/complete
```
