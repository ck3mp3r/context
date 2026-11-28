+++
title = "Install"
description = "Where c5t ships, how to install it, and how to start the server."
weight = 1
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

The primary way to run c5t is the macOS desktop app or the Linux server
container. Both ship one embedded server: the Web UI, the REST API, and the MCP
endpoint all come from it.

## Requirements

- Linux on x86_64 or aarch64, or macOS on Apple silicon.
- 50 MB of disk space and 100 MB of RAM.
- Network access only for git sync and container registries.

## Install the macOS app

Download `context-<version>-aarch64-darwin.dmg` from the
[releases page](https://github.com/ck3mp3r/context/releases). The matching
`.sha256` file sits next to the disk image. The image is built for Apple silicon.

Or install it with Homebrew:

```bash
brew install --cask ck3mp3r/context/context
```

The cask copies `Context.app` into `/Applications` and puts the bundled `c5t`
binary on your `PATH`.

On the first launch `Context.app` starts `context-service` on port 3737 and opens
the Web UI in its window. The app updates itself from GitHub releases, on the
stable channel by default, and the update banner asks for your consent before
anything downloads.

## Run the Linux server container

```bash
docker run -d \
  -p 3737:3737 \
  -v ~/.local/share/c5t:/data \
  --name c5t \
  --restart unless-stopped \
  ghcr.io/ck3mp3r/context:latest
```

The image is multi-arch: `latest` is a manifest that points at the `amd64` and
`arm64` builds. It ships a minimal `git`, so git sync works inside the container.

The Web UI answers at `http://localhost:3737/`, the REST API under
`http://localhost:3737/api/v1/`, and the MCP endpoint at
`http://localhost:3737/mcp`.

## Ports and data

The server answers on four surfaces:

| Surface | Address |
|---------|---------|
| Web UI | `http://localhost:3737/` |
| REST API | `http://localhost:3737/api/v1/*` |
| MCP server | `http://localhost:3737/mcp` |
| WebSocket | `ws://localhost:3737/ws` |

The OpenAPI documentation at `/docs` stays off until you pass `--docs`.

A release build binds port 3737. A debug build binds port 3738 and uses a
separate data directory, so a development server never touches your real data.

The database is created on the first run:

| Build | Database | Skills cache |
|-------|----------|--------------|
| Release | `~/.local/share/c5t/context.db` | `~/.local/share/c5t/skills/` |
| Debug | `~/.local/share/c5t-dev/context.db` | `~/.local/share/c5t-dev/skills/` |

`C5T_SKILLS_DIR` moves the skills cache. The `--skills-dir` flag beats the
variable, and the variable beats the default:

```bash
export C5T_SKILLS_DIR=~/.agents/skills
```

A shared cache such as `~/.agents/skills` lets other agents read the same
skill attachments.

## What's next?

- [Quick Start](@/docs/getting-started/quick-start.md) — open the app, look around, and create your first context.
- [macOS app](@/docs/app/macos.md) — run the app, read the tray, and update it.
- [Other ways to install](@/docs/other-install/cli.md) — the CLI, archives, Nix, and source builds.
