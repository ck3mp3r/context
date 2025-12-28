# context (c5t)

Task management and knowledge tracking system with git-based sync, built for AI-assisted workflows.

## Features

- **Projects & Task Lists** - Organize work hierarchically with M:N relationships
- **Tasks** - Track work with status, priority, subtasks, and timestamps
- **Notes** - Full-text searchable knowledge base with tags and project links
- **Git Sync** - JSONL-based sync via git for cross-machine collaboration
- **MCP Server** - Model Context Protocol integration for AI agents
- **REST API** - HTTP API with OpenAPI documentation
- **SQLite Storage** - Local-first with optional sync

## Documentation

- [Development Guide](docs/development.md) - Setup, building, testing
- [API Reference](docs/api.md) - REST API endpoints
- [MCP Tools](docs/mcp.md) - Model Context Protocol tools
- [Database Schema](docs/schema.md) - SQLite schema and migrations
- [Sync Guide](docs/sync.md) - Git-based cross-machine synchronization

## Architecture

```
context/
├── src/
│   ├── lib.rs              # Shared library
│   ├── bin/
│   │   ├── cli.rs          # CLI binary
│   │   └── api.rs          # API server
│   ├── api/                # REST API (Axum)
│   ├── cli/                # CLI commands
│   ├── db/                 # Database layer (SQLite)
│   ├── mcp/                # MCP server & tools
│   └── sync/               # Git-based sync
├── docs/                   # Documentation
├── scripts/                # Migration & utility scripts
└── data/sql/sqlite/        # Database migrations
```

## License

MIT
