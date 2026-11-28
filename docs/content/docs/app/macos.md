+++
title = "macOS app"
description = "Install the desktop app, run it, read the tray, and update it."
weight = 1
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

The c5t desktop app is a macOS app for Apple silicon. It runs the same service
as the CLI, so the Web UI, the REST API, and the MCP endpoint all come from it.

## Two binaries, one app

| Binary | Role | Activation |
|--------|------|------------|
| `context` | the window: a view over the SPA | regular app, Dock icon, Cmd+Tab |
| `context-service` | the tray, the API server, and the MCP server | accessory, menu bar only |

The window starts no server. When it opens, it looks for a healthy service on
the configured port. When nothing answers, it spawns the `context-service`
binary that sits next to it and waits up to ten seconds for `/health`. When the
service never answers, the window shows an error dialog and exits.

The service owns the tray, the server, and the SPA it serves. A release build
binds port 3737. A debug build binds 3738 and uses `~/.local/share/c5t-dev/`
instead of `~/.local/share/c5t/`.

## Install

```bash
brew install --cask ck3mp3r/context/context
```

The cask copies `Context.app` into `/Applications`. You can also download
`Context_<version>_aarch64.dmg` from the
[releases page](https://github.com/ck3mp3r/context/releases) and copy
`Context.app` out of it.

The app uses the system WebKit framework, which ships with macOS. No other
runtime is needed.

## Run

Open `Context.app` from Finder, Spotlight, or `/Applications`. The first launch
registers the service and, with `start_at_login` on, a launchd agent.

The window behaves like a regular app:

- Closing the window keeps the app running. A Dock click brings the window back.
- Cmd+Q, the application-menu **Quit**, and the Dock Quit end the window process
  only. The service keeps serving.
- The application-menu **Close Window** (Cmd+Shift+W) closes the window. Cmd+W
  belongs to the SPA, which closes the active tab with it.
- Launching the app again focuses the copy that already runs.

The service is a menu-bar agent: no Dock icon, no Cmd+Tab entry. That is why an
MCP client keeps working while the window is closed.

## The tray menu

- **Show Context** — opens the window, or focuses the running one.
- **Settings…** — the same, on the SPA settings page.
- the MCP endpoint row for the configured port, informational.
- the sync row, once `c5t sync init` has run, informational.
- the update row, refreshed by the same poll as the sync row, informational.
- **Quit** — ends the service and the windows it spawned, then stops the server.

A window you started outside the tray is not a child of the service. Tray
**Quit** leaves it running: the SPA shows its connection-lost state, and you
quit that window yourself.

## Settings

Stored settings live in `config.toml`:

| Build | File |
|-------|------|
| Release | `~/.config/c5t/config.toml` |
| Debug | `~/.config/c5t-dev/config.toml` |

The file may hold `host`, `port`, `verbosity`, `enable_docs`, `skills_dir`,
`start_at_login`, `auto_update`, and `update_channel`. An absent key means the
built-in default.

```toml
port = 3737
skills_dir = "/Users/me/.agents/skills"
start_at_login = true
auto_update = true
update_channel = "stable"
```

Precedence, highest first:

1. Command-line flags such as `--port`, `--host`, `--skills-dir`, `-v`, `--docs`.
2. `C5T_SKILLS_DIR`, which acts like an explicit flag for `skills_dir` only.
3. `config.toml`.
4. Built-in defaults.

A stored value applies after the next restart of the process that reads it. The
SPA **Settings** page at `/settings` shows the stored values next to the running
values and marks each field that differs **restart to apply**.

A `config.toml` that cannot be read or parsed, or that carries an out-of-range
value, is reported on stderr and ignored: the service starts on the defaults.

## Start at login

`start_at_login` registers a per-user launchd agent for the service. The service
writes it at startup, so the setting applies on the next service start.

| Part | Value |
|------|-------|
| Plist | `~/Library/LaunchAgents/wtf.kemper.c5t.service.plist` |
| Label | `wtf.kemper.c5t.service` |
| RunAtLoad | `true` |
| KeepAlive | absent: launchd does not respawn the service |

To disable it, set `start_at_login = false` in `config.toml` or clear it on the
Settings page, then restart the service. To undo a registration by hand:

```sh
launchctl bootout gui/$(id -u)/wtf.kemper.c5t.service
rm ~/Library/LaunchAgents/wtf.kemper.c5t.service.plist
```

## Updates

The app updates itself from GitHub releases. The service owns the flow, so an
update keeps running while the window is closed.

1. The service reads the release its channel allows and takes the newest
   version: with `update_channel = "pre-release"` every release is a candidate,
   with `"stable"` only plain versions are.
2. The banner offers the candidate. Nothing downloads until you press Install.
3. On consent the service downloads the signed archive, verifies it against the
   pinned public key, and swaps the bundle atomically. It stops there and keeps
   serving from memory.
4. The restart pill runs the switch: the new service binary starts, the window is
   respawned, and the old process exits.

| Key | Values | Default |
|-----|--------|---------|
| `auto_update` | `true`, `false` — check when the window opens | `true` |
| `update_channel` | `"stable"`, `"pre-release"` | `"stable"` |

A change to either key applies on the next check; no restart is needed. The
Settings page carries a **Check for updates** button and the status of the last
check. Progress is written to `update.json` in the same directory as
`config.toml`.

### Rate limits

GitHub allows 60 anonymous API requests per hour per IP address and 5000
authenticated requests. The update check spends one request. To authenticate
it, hand the service a token:

```sh
launchctl setenv C5T_GITHUB_TOKEN <token>
```

`launchctl setenv` reaches processes started afterwards, so restart the service
next. No scope is needed: the release list of a public repository is readable
with any valid token.

## Troubleshooting

### "Failed to start the context server"

The dialog names the cause, for example:

```
Failed to start the context server: Failed to bind to address 127.0.0.1:3737: Address already in use (os error 48)
```

A release build binds 3737, a debug build binds 3738, and a stored `port`
replaces either. The port is held by a process that does not answer `/health`
with HTTP 200. Find it, stop it, and start the app again.

### The port is already in use

```sh
lsof -nP -iTCP:3737 -sTCP:LISTEN
```

No output means the port is free. Use 3738 for a debug build, or the stored
`port` when `config.toml` names one.

### The window cannot find the service

The window probes the configured port, spawns the sibling `context-service` when
nothing answers, and shows an error dialog when the health wait times out. Check
that the service binary still sits next to the window binary: an app copied out
of `Contents/MacOS` on its own has no sibling to start.

## What's next?

- [Install](@/docs/getting-started/install.md) — the CLI, the container image, and the Nix package.
- [MCP](@/docs/integration/mcp.md) — point an agent at the app's MCP endpoint.
- [Sync](@/docs/integration/sync.md) — the tray sync row, explained.
