# Desktop App

The `context` desktop app ships as two binaries from `crates/context-app`:

| Binary | Role | Activation policy | Identifier |
|--------|------|-------------------|------------|
| `context` | GUI: a view over the SPA | Regular (Dock icon, Cmd+Tab) | `c5t.kemper.wtf` |
| `context-service` | Service: tray, REST and MCP server | Accessory (menu bar only) | `c5t.service.kemper.wtf` |

The GUI starts no server. When it launches it looks for a healthy service on the configured port; when none answers it spawns the sibling `context-service` and waits for `/health`. The service owns the tray, the API server and the SPA it serves.

The service probes `http://127.0.0.1:<port>/health` first. When a server answers with HTTP 200, it attaches to it. Otherwise it starts its own server. The port and the SPA origin depend on the build profile:

| Build | Server port | SPA loaded from |
|-------|-------------|-----------------|
| Debug | 3738 | `trunk serve` on `127.0.0.1:8080` |
| Release | 3737 | the in-process server |

A stored `port` in `config.toml` replaces the profile port; see [Settings and config.toml](#settings-and-configtoml). A debug build never touches the production port by default. A debug build also uses its own data directory, `~/.local/share/c5t-dev/`; a release build uses `~/.local/share/c5t/`.

## Prerequisites

1. Use macOS on Apple silicon. The bundle targets `aarch64`; the dmg name ends in `_aarch64.dmg`.
2. Install the Xcode Command Line Tools. Run `xcode-select -p`. When the command prints a developer directory, the tools are installed. The app uses the system WebKit framework, which ships with macOS.
3. Install [Nix](https://nixos.org/) with flakes enabled, then enter the development shell (see [Development Guide](development.md)):

   ```sh
   nix develop      # or: direnv allow
   ```

   The shell provides the Rust toolchain, `cargo-tauri` 2.12.6, `trunk` 0.22.1, and `tailwindcss` 4.3.4.

   Outside the shell you need the same tools yourself: a Rust toolchain, `cargo install tauri-cli --version ^2.12`, `trunk`, and the Tailwind CSS 4 CLI. The Trunk pre-build hook calls `tailwindcss` and the build fails without it.

4. Confirm the tools:

   ```sh
   cargo tauri --version
   # tauri-cli 2.12.6

   trunk --version
   # trunk 0.22.1

   xcode-select -p
   # /Applications/Xcode.app/Contents/Developer
   ```

5. Add the signing certificate before you build a release. See [Signing and notarization](#signing-and-notarization).

## Dev workflow

The laio `misc` window (`.laio.yaml`) holds three panes side by side: `trunk serve`, `scripts/dev-service`, and the GUI. Run the panes in that order, after `nix develop` (or `direnv allow`).

1. Start the frontend dev server:

   ```sh
   trunk serve
   ```

   It serves the SPA on `http://127.0.0.1:8080/`, proxies `/dev` to `http://localhost:3738/`, and proxies `/dev/ws` to `ws://localhost:3738/ws`. It rebuilds the SPA on every frontend change. Expected output:

   ```
   INFO 📡 serving static assets at -> /
   INFO 📡 proxying /dev -> http://localhost:3738/
   INFO 📡 server listening at: http://127.0.0.1:8080/
   ```

2. Start the service:

   ```sh
   nix develop -c dev-service   # or: scripts/dev-service
   ```

   The helper exports `TAURI_CONFIG='{"identifier":"c5t-dev.service.kemper.wtf"}'` and runs `cargo run --bin context-service`. The service owns the tray and the REST and MCP server. With 3738 free it starts the server there; when a server already answers it attaches to that one. It runs as its own identifier, so its single-instance socket (`/tmp/c5t_dev_service_kemper_wtf_si.sock`) never collides with the GUI's.

3. Start the GUI:

   ```sh
   nix develop -c dev   # or: cargo tauri dev --config '{"identifier":"c5t-dev.kemper.wtf"}'
   ```

   The GUI attaches to the service on 3738 and loads the SPA from the Trunk dev server. It needs both panes: `dev` refuses when nothing serves 3738, because a bare `cargo tauri dev` would otherwise spawn a sibling service baked with the GUI's identifier. Expected output line:

   ```
   Running `/path/to/context/target/debug/context`
   ```

4. Quit the GUI with Cmd+Q or the application menu. The service and its tray keep running; use the tray item **Quit** to stop the pair.

### Dev and production coexistence

The single-instance plugin keys its lock on the app identifier. Each binary has its own, and the dev builds add one more each, so four sockets can exist at once:

| Build | Identifier | Socket |
|-------|------------|--------|
| Production GUI | `c5t.kemper.wtf` | `/tmp/c5t_kemper_wtf_si.sock` |
| Production service | `c5t.service.kemper.wtf` | `/tmp/c5t_service_kemper_wtf_si.sock` |
| Dev GUI | `c5t-dev.kemper.wtf` | `/tmp/c5t_dev_kemper_wtf_si.sock` |
| Dev service | `c5t-dev.service.kemper.wtf` | `/tmp/c5t_dev_service_kemper_wtf_si.sock` |

The identifier is baked at compile time from `TAURI_CONFIG`, and that variable applies to the whole crate. The two dev commands therefore differ: `dev` passes the GUI identifier, `dev-service` exports the service identifier. A second instance of any surface exits immediately and the first one takes over.

| First app running | Second app started | Result |
|-------------------|--------------------|--------|
| Production GUI | Dev GUI | Both run. Separate identifiers, separate sockets. |
| Production service | Dev service | Both run. Separate identifiers, separate sockets. |
| Dev GUI | Dev GUI | The second instance prints `Running .../target/debug/context` and exits within a few seconds. |
| Production GUI | Production GUI | The second instance exits and the first shows its window. |

The service and the GUI share a build profile, not a lock: a dev GUI attaches to whichever server answers on the dev port.

### Runtime behavior

The GUI is a regular app: it has a Dock icon and a Cmd+Tab entry, and it is a pure view over the service. Closing its window keeps the GUI alive, and a Dock click brings the window back.

- Cmd+Q, the application-menu **Quit**, and the Dock's Quit end the GUI process only. The service keeps serving.
- The close button, Cmd+W, and the application-menu **Close Window** close the window. The GUI stays running; a Dock click reopens the window through the reopen event.
- Launching the GUI again focuses the running one through its single-instance handler.

When a healthy service does not answer on the configured port, the GUI spawns the sibling `context-service` and waits up to ten seconds for `/health`. When the service never answers, the GUI shows an error dialog and exits.

The service is a macOS accessory agent: no Dock icon, no Cmd+Tab entry. It owns the tray, and the window is a transient view of it, because MCP clients keep using the server while the window is closed.

The tray menu holds:

- **Show context** — launches the GUI, or focuses the running one.
- **Settings…** — the same, on the SPA settings page when the GUI has to be spawned.
- the MCP endpoint row for the configured port, informational.
- the sync row, after `c5t sync init`, informational.
- **Quit** — ends the service and the GUIs the service spawned, then stops the server.

A GUI that the user started outside the tray — from Finder, a terminal, or the dev panes — is not a child of the service. The tray **Quit** leaves it running: the SPA shows its connection-lost state, and the user quits that GUI. The service does not respawn itself or the GUI this iteration.

### Headless server

To work on the server without the tray, stop the service pane, then run the server in a spare pane:

```sh
cargo run --bin c5t api
```

A debug build binds `0.0.0.0:3738` by default and uses the same dev database, `~/.local/share/c5t-dev/context.db`. Run one server per port: when the headless server already runs, the service attaches to it, and quitting the service leaves 3738 taken.

This command fails when 3738 is in use. Expected error:

```
Error:   × Failed to bind to address 0.0.0.0:3738: Address already in use
```

Frontend-only work does not need either binary. See [Frontend Architecture](frontend.md).

## Settings and config.toml

Stored settings live in `config.toml` under the profile configuration directory:

| Build | File |
|-------|------|
| Debug | `~/Library/Application Support/c5t-dev/config.toml` |
| Release | `~/Library/Application Support/c5t/config.toml` |

`XDG_CONFIG_HOME` replaces the macOS configuration root, and `c5t api --home` replaces the root for one run.

The file holds any of `host`, `port`, `verbosity`, `enable_docs`, `skills_dir`, and `start_at_login`. An absent key means "use the built-in default"; the service resolves the file over the defaults at startup.

Precedence, highest first:

1. CLI flags (`c5t api --port …`, `--host`, `--skills-dir`, `-v`, `--docs`).
2. `C5T_SKILLS_DIR`, which behaves like an explicit flag for `skills_dir` only.
3. `config.toml`.
4. Built-in defaults: port 3738 (debug) or 3737 (release), host `0.0.0.0` for the CLI and loopback for the service, verbosity 0, docs off, `skills_dir` under the data directory, `start_at_login` on.

A stored value applies after the next restart of the process that reads it. The SPA **Settings** page (`/settings`) shows the stored values, the built-in defaults and what the running process uses; a field that differs from the running process is marked **restart to apply**.

A `config.toml` that cannot be parsed, or that carries an out-of-range value, is logged and ignored: the service starts on the built-in defaults.

## Start at login

`start_at_login` registers a per-user launchd agent for the service. The service writes it at startup, so the setting applies on the next service start.

| Part | Value |
|------|-------|
| Plist | `~/Library/LaunchAgents/wtf.kemper.c5t.service.plist` |
| Label | `wtf.kemper.c5t.service` |
| Program | the running service binary, absolute path |
| RunAtLoad | `true` |
| KeepAlive | absent: launchd does not respawn the service |

To disable it, set `start_at_login = false` in `config.toml` (or clear it on the Settings page) and restart the service: it unregisters the agent and removes the plist. To undo a registration by hand:

```sh
launchctl bootout gui/$(id -u)/wtf.kemper.c5t.service
rm ~/Library/LaunchAgents/wtf.kemper.c5t.service.plist
```

A plist or `launchctl` failure is logged and non-fatal: the service keeps serving.

## Release build

The release build produces `context.app` and the dmg `context_<version>_aarch64.dmg`. Both land under `target/release/bundle/`. `Contents/MacOS/` holds three binaries: `context` (the bundle main, from `default-run`), `context-service`, and `c5t` (the CLI, staged as an `externalBin` sidecar); the bundler signs all three.

The CLI sidecar is `bundle.externalBin` in `crates/context-app/tauri.conf.json`. The bundler looks for `<path>-aarch64-apple-darwin` and copies it into `Contents/MacOS/` under its own name, so the `release` helper builds and stages it before `cargo tauri build`.

A release build binds 3737 for its in-process server unless `config.toml` stores another port. The port must be free, or answered by a healthy server the service can attach to. A debug build uses 3738.

Run the `release` helper from the development shell. It builds, notarizes, and staples in one step:

```sh
export APPLE_API_KEY=<key id>
export APPLE_API_ISSUER=<issuer id>
export APPLE_API_KEY_PATH=<path to the .p8 key>

release
```

Expected output tail:

```
dmg: target/release/bundle/dmg/context_0.8.0_aarch64.dmg
Verify:
  xcrun stapler validate target/release/bundle/dmg/context_0.8.0_aarch64.dmg
  spctl --assess --type open --verbose --context context:primary-signature target/release/bundle/dmg/context_0.8.0_aarch64.dmg
```

The helper runs these steps in order:

1. Refuse to run off macOS.
2. Refuse to start when `APPLE_API_KEY`, `APPLE_API_ISSUER`, or `APPLE_API_KEY_PATH` is missing.
3. Refuse to start when `lsof` is not on `PATH`.
4. Refuse to start when port 8080 has a listener (a live `trunk serve`).
5. Run `trunk build --release`.
6. Run `cargo build --release -p context-cli`.
7. Copy `target/release/c5t` to `target/release/c5t-aarch64-apple-darwin` (the name the bundler looks for).
8. Run `cargo tauri build`.
9. Resolve exactly one `target/release/bundle/dmg/context*_aarch64.dmg`.
10. Run `xcrun notarytool submit <dmg> --key <key path> --key-id <key id> --issuer <issuer id> --wait`.
11. Run `xcrun stapler staple <dmg>`.

`cargo tauri build` signs the app with the Developer ID identity. When the three `APPLE_API_*` variables are exported, it also notarizes and staples the `.app` **only**. It codesigns the dmg but never submits it to the notary service. Without the helper's submit and staple steps the dmg is signed but not notarized, and `xcrun stapler validate` fails on it.

`trunk build --release` must run before `cargo tauri build`. The release binary embeds `crates/context-server/dist/` at compile time. Only a Trunk release build fills that directory with the real SPA; without it the binary embeds the stub page.

### Stop `trunk serve` first

Stop the `trunk serve` pane, and any other dev server on 8080, before a release build. A live dev server rewrites `crates/context-server/dist/` on every frontend change (`Trunk.toml` serve output and watch), while the release embed reads that directory at compile time (`crates/context-server/src/api/static_assets.rs`). A build that races the dev server can embed a half-written SPA and fail intermittently. The `release` helper enforces the rule and refuses to start:

```
release: port 8080 is in use by a running 'trunk serve'
release: trunk serve rewrites dist/ while rust-embed reads it at compile
release: time, so the bundle can embed a half-written SPA. Stop the trunk
release: pane, then rerun release.
```

## Signing and notarization

Release builds use the Developer ID Application identity from `crates/context-app/tauri.conf.json` (`bundle.macOS.signingIdentity`) with the hardened runtime enabled. Notarization needs a Developer ID certificate and an App Store Connect API key.

### Certificate

Import the Developer ID Application certificate into the login keychain. List the available identities:

```sh
security find-identity -v -p codesigning
```

Expected output:

```
1) <hash> "Developer ID Application: Christian Kemper (ZESFXRA4R8)"
   1 valid identities found
```

The release build needs a matching identity in this list.

### App Store Connect API key

Create a key in App Store Connect under Users and Access → Integrations. Export three variables before the release:

- `APPLE_API_KEY` — the Key ID.
- `APPLE_API_ISSUER` — the Issuer ID.
- `APPLE_API_KEY_PATH` — the path to the downloaded `AuthKey_<KEY_ID>.p8` file.

`cargo tauri build` uses them for the app, and the `release` helper passes them to `notarytool` for the dmg.

### Check the result

Run both checks on the produced dmg after a successful `release`:

```sh
xcrun stapler validate target/release/bundle/dmg/context_0.8.0_aarch64.dmg
# Processing: /path/to/context_0.8.0_aarch64.dmg
# The validate action worked!
```

```sh
spctl --assess --type open --verbose --context context:primary-signature target/release/bundle/dmg/context_0.8.0_aarch64.dmg
# /path/to/context_0.8.0_aarch64.dmg: accepted
# source=Notarized Developer ID
```

Do not use the bare form `spctl --assess --type open <dmg>`. A disk image needs the assessment context: the bare form rejects a correctly notarized dmg with exit status 3 and `rejected`.

### Check the helper preflights

These checks need no real credentials and no build:

```sh
# Missing credentials: the helper stops before any build.
nix develop -c release
# release: missing notarization credentials: APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH
# release: export APPLE_API_KEY, APPLE_API_ISSUER and APPLE_API_KEY_PATH first
# exit status 1
```

```sh
# Missing lsof: keep only /usr/bin on PATH so the Darwin guard passes.
export APPLE_API_KEY=dummy APPLE_API_ISSUER=dummy APPLE_API_KEY_PATH=/tmp/dummy.p8
nix develop -c env PATH=/usr/bin "$(nix develop -c command -v release | tail -n 1)"
# release: lsof is required but not found in PATH
# exit status 1
```

Two traps, both confirmed:

- The literal form `nix develop -c env PATH=/usr/bin release` exits with status 127 and `env: 'release': No such file or directory`. The overridden `PATH` decides where `env` looks for `release`, so pass the absolute path.
- Leaving out `| tail -n 1` exits with status 126 and `File name too long`. Every `nix develop -c` invocation prints the dev-shell banner first, so the command substitution captures the banner plus the path. Keep `| tail -n 1`.

On macOS `lsof` lives in `/usr/sbin` and `uname` in `/usr/bin`, so `PATH=/usr/bin` passes the Darwin check and fails the `lsof` check closed.

## Troubleshooting

### The service reports "Failed to start the context server"

The dialog text names the cause, for example:

```
Failed to start the context server: Failed to bind to address 127.0.0.1:3738: Address already in use (os error 48)
```

A debug build binds 3738; a release build binds 3737; a stored `port` in `config.toml` replaces either. The port is held by a process that does not answer `/health` with HTTP 200. Find the process, stop it, and start the service again. To see the same error from the command line, run `cargo run --bin c5t api` while the port is taken; it exits with status 1 and `Failed to bind to address 0.0.0.0:3738: Address already in use`.

When the GUI shows this text, the service it spawned could not start. Start the service in its own pane to read the error on its stdout.

### The port is already in use

Find the listener:

```sh
lsof -nP -iTCP:3738 -sTCP:LISTEN
# COMMAND   PID      USER   FD   TYPE ... NODE NAME
# c5t     69904 christian   12u  IPv4 ...  TCP *:3738 (LISTEN)
```

No output means the port is free. Use the same command with 3737 for a release build, or with the stored `port` when `config.toml` names one.

The service and the headless dev server share the configured port: a debug pair uses 3738, a release pair 3737. Run one server per port: quit the extra service from the tray, or stop the extra server. The service attaches to a healthy server instead of starting a second one.

### The GUI cannot find the service

The GUI probes the configured port, spawns the sibling `context-service` when nothing answers, and shows an error dialog when the health wait times out. Two cases are worth checking:

- The spawned sibling must be next to the GUI binary. A GUI copied out of `target/debug` or out of `Contents/MacOS` has no sibling to start.
- Another process holds the port without answering `/health`. Stop it and launch the GUI again.

### The window is blank or shows "The frontend assets are not embedded in this build"

The binary embedded the stub `dist/index.html` that `crates/context-server/build.rs` creates for fresh clones. Stop `trunk serve`, run `trunk build --release`, then rebuild the app (`release`, or `cargo tauri build`). A debug build reads `dist/` at runtime, but its window loads the SPA from `trunk serve` on 8080, so keep the dev server running for debug work.

### The release build fails intermittently while `trunk serve` runs

This is the race described in [Stop `trunk serve` first](#stop-trunk-serve-first). Stop the dev server pane and rerun. The `release` helper refuses to start while port 8080 has a listener.

### The dmg Gatekeeper check fails

Use the context form shown in [Check the result](#check-the-result). The bare `spctl --assess --type open <dmg>` form rejects a notarized disk image.
