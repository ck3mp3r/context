# Development Guide

## Prerequisites

- [Nix](https://nixos.org/) with flakes enabled
- [direnv](https://direnv.net/) (optional, for automatic shell activation)

## Setup

```sh
# Clone repository
git clone https://github.com/ck3mp3r/context.git
cd context

# Enter development environment
direnv allow  # or: nix develop
```

## Building

```sh
# Build all binaries
cargo build

# Build for release (optimized)
cargo build --release
```

## Running

```sh
# CLI
cargo run --bin c5t -- --help

# API server
cargo run --bin c5t-api

# API with trace logging
RUST_LOG=trace cargo run --bin c5t-api

# API on custom port
C5T_PORT=8080 cargo run --bin c5t-api
```

## Testing

```sh
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture
```

## Code Quality

```sh
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Lint
cargo clippy

# Lint in CI mode (fail on warnings)
cargo clippy -- -D warnings
```

## Database

### Location
- Default: `~/.local/share/c5t/context.db`

### Migrations

Migrations are managed by SQLx and located in `data/sql/sqlite/migrations/`.

They run automatically on application startup.

### Schema Changes

When modifying the schema:
1. Create new migration file in `data/sql/sqlite/migrations/`
2. Name format: `YYYYMMDDHHMMSS_description.sql`
3. Test migration on development database
4. Update `docs/schema.md` to reflect changes

## Project Structure

```
context/
├── src/
│   ├── lib.rs              # Shared library
│   ├── bin/
│   │   ├── cli.rs          # CLI binary
│   │   └── api.rs          # API server
│   ├── api/                # REST API (Axum)
│   │   ├── handlers/       # HTTP handlers
│   │   ├── routes.rs       # Route definitions
│   │   └── state.rs        # Shared state
│   ├── cli/                # CLI commands
│   ├── db/                 # Database layer
│   │   ├── sqlite/         # SQLite implementation
│   │   ├── repository.rs   # Repository trait
│   │   └── models.rs       # Data models
│   ├── mcp/                # MCP server
│   │   ├── server.rs       # Server coordinator
│   │   ├── service.rs      # Service layer
│   │   └── tools/          # Tool implementations
│   └── sync/               # Git-based sync
│       ├── manager.rs      # Sync manager
│       ├── git.rs          # Git operations
│       └── jsonl.rs        # JSONL format
├── data/sql/sqlite/        # Database migrations
├── docs/                   # Documentation
├── scripts/                # Utility scripts
└── nix/                    # Nix configuration
```

## Release Build

The release profile is optimized for binary size:

```toml
[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit
panic = "abort"     # No unwinding
strip = true        # Strip symbols
```

Build release binary:
```sh
cargo build --release
./target/release/c5t --version
./target/release/c5t-api
```
