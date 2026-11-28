+++
title = "Install the CLI"
description = "Install the c5t CLI with Homebrew, a release archive, or Nix, and start its server."
weight = 1
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

The CLI is the server without the window: the `c5t` binary serves the Web UI,
the REST API, and the MCP endpoint, and it carries the client commands that talk
to them.

## Homebrew formula

The formula installs the `c5t` CLI on macOS Apple silicon, Linux x86_64, and
Linux ARM64:

```bash
brew tap ck3mp3r/context
brew install ck3mp3r/context/context
```

The same tap carries the macOS app as a cask. Add `--cask` to the install line
to get the app instead of the CLI.

## Start the CLI server

```bash
c5t api
```

The server binds port 3737 and serves the Web UI, the REST API, and the MCP
server from one process. Leave it running in its own terminal.

Flags change where it listens, which data it uses, and how much it logs:

```bash
# Custom port
c5t api -p 8080

# Custom data directory
c5t api --home /tmp/c5t-data

# More logging: -v info, -vv debug, -vvv trace
c5t api -vv
```

The OpenAPI documentation at `/docs` stays off until you pass `--docs`.

## Release archive

Download an archive from the [releases page](https://github.com/ck3mp3r/context/releases).
The platforms are `x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`:

```bash
# Download and extract (replace VERSION and PLATFORM)
tar -xzf context-VERSION-PLATFORM.tgz

# Install
sudo mv c5t /usr/local/bin/
chmod +x /usr/local/bin/c5t
```

Each archive ships a matching `.sha256` file next to it.

## Nix

```bash
nix profile install github:ck3mp3r/context
```

This requires [Nix](https://nixos.org/) with flakes enabled.

## What's next?

- [Quick Start](@/docs/getting-started/quick-start.md) — a project, a task list, and your first tasks.
- [CLI reference](@/docs/usage/cli.md) — every command, plus repositories, skills, and output formats.
