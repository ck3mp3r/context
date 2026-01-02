# Context

Task management and knowledge tracking system with git-based sync, built for AI-assisted workflows.

**CLI Command:** `c5t` (short for "context")

## Features

- **Projects & Task Lists** - Organize work hierarchically with M:N relationships
- **Tasks** - Track work with status, priority, subtasks, and timestamps
- **Notes** - Full-text searchable knowledge base with tags and project links
- **Git Sync** - JSONL-based sync via git for cross-machine collaboration
- **MCP Server** - Model Context Protocol integration for AI agents
- **REST API** - HTTP API with OpenAPI documentation
- **Web UI** - Leptos-based WASM frontend embedded in single binary
- **SQLite Storage** - Local-first with optional sync

## Quick Start

**Run the API server (includes Web UI, REST API, and MCP server):**

```sh
c5t api --port 3737
```

Then access:
- **Web UI**: http://localhost:3737/
- **REST API**: http://localhost:3737/api/v1/*
- **OpenAPI Docs**: http://localhost:3737/docs
- **MCP Server**: http://localhost:3737/mcp

**Git Sync** - Sync your data across machines using a private git repository:

```sh
# 1. Create private git repo (GitHub example)
gh repo create context-sync --private

# 2. Initialize sync
c5t sync init git@github.com:username/context-sync.git

# 3. Export your data (run daily or when done working)
c5t sync export -m "End of day sync"

# 4. On another machine: import data
c5t sync import

# 5. Check sync status anytime
c5t sync status
```

**Multi-machine workflow:**
```sh
# Machine A (end of day)
c5t sync export -m "Updated project tasks"

# Machine B (start of day)
c5t sync import     # Pull changes from Machine A
# ... work on tasks ...
c5t sync export     # Push changes back

# Machine A (next day)
c5t sync import     # Pull changes from Machine B
```

See [Sync Guide](docs/sync.md) for SSH setup, conflict resolution, and troubleshooting.

## Documentation

- [Development Guide](docs/development.md) - Setup, building, testing
- [API Reference](docs/api.md) - REST API endpoints
- [MCP Tools](docs/mcp.md) - Model Context Protocol tools
- [Frontend Architecture](docs/frontend.md) - Leptos WASM UI & build process
- [Database Schema](docs/schema.md) - SQLite schema and migrations
- [Sync Guide](docs/sync.md) - Git-based cross-machine synchronization

## Architecture

**Single unified binary** (`c5t`) with embedded WASM frontend:

```
context/
├── src/
│   ├── lib.rs              # Shared library
│   ├── bin/
│   │   └── cli.rs          # Unified CLI binary
│   ├── api/                # REST API (Axum) + embedded assets
│   ├── cli/                # CLI commands (api, sync, task, etc.)
│   ├── db/                 # Database layer (SQLite)
│   ├── mcp/                # MCP server & tools
│   ├── sync/               # Git-based sync
│   └── frontend/           # Leptos WASM UI (embedded via rust-embed)
├── docs/                   # Documentation
├── scripts/                # Migration & utility scripts
└── data/sql/sqlite/        # Database migrations
```

The `c5t api` command serves:
- **Web UI** at `/` (Leptos WASM SPA, embedded in binary)
- **REST API** at `/api/v1/*` (Axum handlers)
- **MCP Server** at `/mcp` (Model Context Protocol)
- **OpenAPI Docs** at `/docs` (Swagger UI)

## License

MIT
