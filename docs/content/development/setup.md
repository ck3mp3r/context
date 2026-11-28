+++
title = "Development setup"
description = "Set up the Nix dev shell, build and run c5t, and run the test and lint commands."
weight = 1
draft = false
template = "development/page.html"

[extra]
toc = true
+++

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
# Show help
cargo run --bin c5t -- --help

# API server
cargo run --bin c5t -- api

# API with trace logging
RUST_LOG=trace cargo run --bin c5t -- api

# API on custom port
cargo run --bin c5t -- api --port 8080
```

## Testing

```sh
# Run all tests
cargo nextest run --workspace

# Run doctests (nextest does not run them)
cargo test --doc

# Run the CI profile
cargo nextest run --profile ci
```

## Code Quality

```sh
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Lint
cargo clippy --all-targets

# Lint in CI mode (workspace lints already deny warnings)
cargo clippy --all-targets
```

## Database

### Location
- Default: `~/.local/share/c5t/context.db`

### Migrations

Migrations are managed by SQLx and located in `crates/context-db/data/sql/sqlite/migrations/`.

They run automatically on application startup.

### Schema Changes

When modifying the schema:
1. Create new migration file in `crates/context-db/data/sql/sqlite/migrations/`
2. Name format: `YYYYMMDDHHMMSS_description.sql`
3. Test migration on development database
4. Update `docs/schema.md` to reflect changes

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
./target/release/c5t api
```

## What's next?

- [Code coverage](@/development/coverage.md) — measure the lines, regions, and functions the tests reach.
- [Frontend architecture](@/development/frontend.md) — how the Leptos and Thaw UI app is built and embedded.
- [Thaw UI integration](@/development/thaw-integration.md) — which Thaw components c5t uses, and where.
