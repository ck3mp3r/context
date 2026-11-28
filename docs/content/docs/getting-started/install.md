+++
title = "Install"
description = "Where c5t ships, how to install it, and how to start the server."
weight = 1
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

c5t ships as a CLI, a macOS desktop app, a container image, and a Nix flake.
One binary serves every interface: the CLI, the REST API, the MCP server, and
the Web UI.

## Requirements

- Linux on x86_64 or aarch64, or macOS on Apple silicon.
- 50 MB of disk space and 100 MB of RAM.
- Network access only for git sync and container registries.

## Install the CLI

### Homebrew

The formula installs the `c5t` CLI on macOS Apple silicon, Linux x86_64, and
Linux ARM64:

```bash
brew tap ck3mp3r/context
brew install ck3mp3r/context/context
```

### Nix

```bash
nix profile install github:ck3mp3r/context
```

This requires [Nix](https://nixos.org/) with flakes enabled.

### Container

```bash
docker run -d \
  -p 3737:3737 \
  -v ~/.local/share/c5t:/data \
  --name c5t \
  --restart unless-stopped \
  ghcr.io/ck3mp3r/context:latest
```

Git sync is not available inside a container.

### Release archive

Download an archive from the [releases page](https://github.com/ck3mp3r/context/releases).
The platforms are `x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`:

```bash
# Download and extract (replace VERSION and PLATFORM)
tar -xzf context-VERSION-PLATFORM.tgz

# Install
sudo mv c5t /usr/local/bin/
chmod +x /usr/local/bin/c5t
```

### Build from source

The repository ships a Nix development shell with the Rust toolchain and the
frontend tools:

```bash
nix develop
cargo build --release
./target/release/c5t api
```

## Install the macOS app

The cask installs `Context.app` into `/Applications` on macOS Apple silicon:

```bash
brew install --cask ck3mp3r/context/context
```

The app bundles the same service as the CLI. Read the
[macOS app](@/docs/app/macos.md) page for the two-binary model, updates, and
the tray menu.

## Start the server

```bash
c5t api
```

The server answers on four surfaces:

| Surface | Address |
|---------|---------|
| Web UI | `http://localhost:3737/` |
| REST API | `http://localhost:3737/api/v1/*` |
| MCP server | `http://localhost:3737/mcp` |
| WebSocket | `ws://localhost:3737/ws` |

The OpenAPI documentation at `/docs` stays off until you pass `--docs`.

## Ports and data

A release build binds port 3737. A debug build binds port 3738 and uses a
separate data directory, so a development server never touches your real data.

```bash
# Custom port
c5t api -p 8080

# Custom data directory
c5t api --home /tmp/c5t-data

# More logging: -v info, -vv debug, -vvv trace
c5t api -vv
```

The database is created on the first run:

| Build | Database | Skills cache |
|-------|----------|--------------|
| Release | `~/.local/share/c5t/context.db` | `~/.local/share/c5t/skills/` |
| Debug | `~/.local/share/c5t-dev/context.db` | `~/.local/share/c5t-dev/skills/` |

`C5T_SKILLS_DIR` moves the skills cache. The `--skills-dir` flag beats the
variable, and the variable beats the default:

```bash
export C5T_SKILLS_DIR=~/.agents/skills
c5t api
```

A shared cache such as `~/.agents/skills` lets other agents read the same
skill attachments.

## What's next?

- [Quick Start](@/docs/getting-started/quick-start.md) — a project, a task list, and your first tasks.
- [CLI](@/docs/usage/cli.md) — the full command surface.
- [macOS app](@/docs/app/macos.md) — install, run, and update the desktop app.
