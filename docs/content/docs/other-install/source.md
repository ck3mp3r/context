+++
title = "Build from source"
description = "Build the c5t binary from the repository with the Nix dev shell and a release cargo build."
weight = 2
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

A source build gives you the tree you cloned: the CLI, the server, and the SPA
embedded in one binary.

## Prerequisites

[Nix](https://nixos.org/) with flakes enabled. The dev shell carries the Rust
toolchain, `trunk`, and `wasm-bindgen`, so nothing else is needed.

## Build

```bash
git clone https://github.com/ck3mp3r/context.git
cd context
nix develop
cargo build --release
```

The server embeds the SPA, and `rust-embed` bakes it in at compile time. Build
the UI before the binary, or the binary carries the stub page instead:

```bash
trunk build --release
cargo build --release
```

The dev-shell `build` helper runs both commands in that order.

## Run

```bash
./target/release/c5t api
```

The release profile optimizes for size: `opt-level = "z"`, link-time
optimization, one codegen unit, `panic = "abort"`, and stripped symbols. A
release build takes longer than a debug build and produces a small binary.

A release build binds port 3737 and uses `~/.local/share/c5t/`; a debug build
binds 3738 and uses `~/.local/share/c5t-dev/`.

## What's next?

- [Quick Start](@/docs/getting-started/quick-start.md) — a project, a task list, and your first tasks.
- [Development setup](@/development/setup.md) — the dev shell, the test commands, and the build helpers.
