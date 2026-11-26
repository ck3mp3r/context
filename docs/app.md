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

5. Add the signing certificate before you build a release, and generate the updater signing key before CI can publish updates. See [Signing and notarization](#signing-and-notarization) and [Updates](#updates).

## Dev workflow

The laio `misc` window (`.laio.yaml`) holds three panes side by side: the preview, `scripts/dev-service`, and the GUI. Run them in that order, after `nix develop` (or `direnv allow`).

1. Start the frontend preview:

   ```sh
   cp scripts/c5t-trunk-serve.plist ~/Library/LaunchAgents/    # once per machine
   launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/c5t-trunk-serve.plist
   ```

   Stop it with `launchctl bootout gui/$(id -u)/c5t-trunk-serve`, restart it with `launchctl kickstart -k gui/$(id -u)/c5t-trunk-serve`. The repo-tracked plist carries `RunAtLoad` and no `KeepAlive`, so launchd starts one `trunk serve` and never respawns it: one start, one process, and a stop leaves nothing behind. The `misc` window's `trunk serve` pane binds the same port, so run the agent or the pane, never both.

   It serves the SPA on `http://127.0.0.1:8080/`, proxies `/dev` to `http://localhost:3738/`, and proxies `/dev/ws` to `ws://localhost:3738/ws`. It rebuilds the SPA on every frontend change, and the log is `/tmp/trunk-serve.log`. [frontend.md](frontend.md#development-mode) lists the `PATH` the agent needs for the `tailwindcss` hook. Expected output:

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
- The close button and the application-menu **Close Window** (Cmd+Shift+W) close the window. The GUI stays running; a Dock click reopens the window through the reopen event. Cmd+W belongs to the SPA, which closes the active tab with it and never the window.
- Launching the GUI again focuses the running one through its single-instance handler.

When a healthy service does not answer on the configured port, the GUI spawns the sibling `context-service` and waits up to ten seconds for `/health`. When the service never answers, the GUI shows an error dialog and exits.

The service is a macOS accessory agent: no Dock icon, no Cmd+Tab entry. It owns the tray, and the window is a transient view of it, because MCP clients keep using the server while the window is closed.

The tray menu holds:

- **Show Context** — launches the GUI, or focuses the running one.
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
| Debug | `~/.config/c5t-dev/config.toml` |
| Release | `~/.config/c5t/config.toml` |

`XDG_CONFIG_HOME` does not move the configuration root. The root is `~/.config` on every platform: macOS, Windows and Linux alike. Resolution is identical whether the variable is unset, set to `~/.config`, or set to a bespoke path. The app and `context-service` start without the shell's environment while a terminal CLI inherits it, so honoring the variable would give one profile two `config.toml` files. On Windows the root is `%USERPROFILE%\.config\{profile}` when `HOME` is set — the MSYS2/Git Bash convention the data root already uses — and the temp-directory fallback when `HOME` is unset. `c5t api --home` replaces the root for one run.

No supported override moves the root. To add one, read `XDG_CONFIG_HOME` in `default_config_home` in `crates/context-core/src/paths.rs`, before the `$HOME/.config` fallback: that is a one-line change.

Older builds wrote the platform-native root: `~/Library/Application Support` on macOS, `%APPDATA%` on Windows, `~/.config` elsewhere. A legacy `config.toml` there (debug: `c5t-dev`) is copied into `~/.config` when no `~/.config` file exists. The copy runs once: an existing `~/.config` file wins and is never overwritten. A missing legacy file, an unreadable one, or a failed write is silent, so startup never fails on migration.

The file holds any of `host`, `port`, `verbosity`, `enable_docs`, `skills_dir`, and `start_at_login`. An absent key means "use the built-in default"; the service resolves the file over the defaults at startup.

Precedence, highest first:

1. CLI flags (`c5t api --port …`, `--host`, `--skills-dir`, `-v`, `--docs`).
2. `C5T_SKILLS_DIR`, which behaves like an explicit flag for `skills_dir` only.
3. `config.toml`.
4. Built-in defaults: port 3738 (debug) or 3737 (release), host `0.0.0.0` for the CLI and loopback for the service, verbosity 0, docs off, `skills_dir` under the data directory, `start_at_login` on.

### Environment overrides

The same order holds for every override: an explicit flag or an environment variable beats `config.toml`, and `config.toml` beats the built-in defaults.

| Override | Effect | Position |
|----------|--------|----------|
| `--api-url` | Base URL the CLI client calls | First: beats `C5T_API_URL`, the stored `port` and the profile URL |
| `C5T_API_URL` | Base URL the CLI client calls | Second: used when `--api-url` is absent; beats the stored `port` and the profile URL |
| `C5T_SKILLS_DIR` | Skills cache directory | A flag for `skills_dir` only: `--skills-dir` beats it, and it beats the stored `skills_dir` |
| `C5T_PURGE_RETENTION_DAYS` | Days a tombstone is kept before purge | Beats the built-in 30 days; the file holds no such key |
| `C5T_GITHUB_TOKEN` | Bearer token for the update check's release-list request | Beats an anonymous request; the file holds no such key |

A stored value applies after the next restart of the process that reads it. The SPA **Settings** page (`/settings`) shows the stored values, the built-in defaults and what the running process uses; a field that differs from the running process is marked **restart to apply**.

A `config.toml` that cannot be read or parsed, that is a directory, or that carries an out-of-range value is reported on stderr and ignored: the service starts, and the CLI client dials, on the built-in defaults. The CLI client prints `c5t: ignoring settings file <path>: <error>`, so a truncated file left by an interrupted write is visible instead of silent. A missing file, the normal first run, and a file with absent keys are silent.

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

The release build produces `Context.app` and the dmg `Context_<version>_aarch64.dmg`. Both land under `target/release/bundle/`. `Contents/MacOS/` holds three binaries: `Context` (the bundle main: `mainBinaryName` renames the cargo binary `context`), `context-service`, and `c5t` (the CLI, staged as an `externalBin` sidecar); the bundler signs all three.

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
dmg: target/release/bundle/dmg/Context_0.8.0_aarch64.dmg
Verify:
  xcrun stapler validate target/release/bundle/dmg/Context_0.8.0_aarch64.dmg
  spctl --assess --type open --verbose --context context:primary-signature target/release/bundle/dmg/Context_0.8.0_aarch64.dmg
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
9. Resolve exactly one `target/release/bundle/dmg/Context*_aarch64.dmg`.
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
xcrun stapler validate target/release/bundle/dmg/Context_0.8.0_aarch64.dmg
# Processing: /path/to/Context_0.8.0_aarch64.dmg
# The validate action worked!
```

```sh
spctl --assess --type open --verbose --context context:primary-signature target/release/bundle/dmg/Context_0.8.0_aarch64.dmg
# /path/to/Context_0.8.0_aarch64.dmg: accepted
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

## Updates

The app updates itself from GitHub releases. The service owns the flow, so an update keeps running while the window is closed: the service checks when the GUI opens and when the user asks, and installs only after a click in the SPA banner.

### How an update runs

1. The service resolves the release that its channel allows — with the pre-release channel on, every release is a candidate; with it off, only stable releases — and takes the newest version. Versions follow semver, so a build-suffix pre-release such as `0.8.1-build.412` sorts below `0.8.1` and is never offered over it, and it sorts above `0.8.1-build.99`, because a numeric build identifier compares by value.
2. The SPA banner offers the candidate. Nothing downloads until Install is pressed.
3. On consent the service downloads the signed `Context.app.tar.gz`, verifies it against the pinned public key, and swaps the bundle atomically. It stops there: the state becomes `ready`, the bundle waits on disk, and the running service keeps serving it.
4. Restart, from the pill CTA, runs the sequence: the service starts the new service binary with a one-second delay, terminates and respawns the GUI, then exits. The old process releases port 3737 and the single-instance socket while the child still sleeps, so one instance always owns them. The launchd agent is not touched: the detached child owns the session, and at the next login the registered agent starts the new binary by itself.
5. Progress lives in `update.json` in the profile configuration directory:

   | Build | State file |
   |-------|------------|
   | Debug | `~/.config/c5t-dev/update.json` |
   | Release | `~/.config/c5t/update.json` |

### Settings

`config.toml` carries two keys, both editable on the Settings page. A change applies on the next check; no restart is needed.

| Key | Values | Default |
|-----|--------|---------|
| `auto_update` | `true`, `false` — check when the GUI opens | `true` |
| `update_channel` | `"stable"`, `"pre-release"` | `"stable"` |

The Settings page also carries a Check for updates button and the status line of the last check.

### Authenticated fetch

GitHub budgets anonymous API requests at 60 per hour per IP and authenticated requests at 5000 per hour. The update check spends one request, so the anonymous budget is the one it trips first on a shared address. The service reads a token from `C5T_GITHUB_TOKEN` and sends it as a bearer header on the release-list request; an unset or blank variable keeps the check anonymous.

Create a personal access token and hand it to the launchd session the service starts in:

```sh
launchctl setenv C5T_GITHUB_TOKEN <token>
```

`launchctl setenv` reaches processes started afterwards, so restart the service: `launchctl bootout gui/$(id -u)/wtf.kemper.c5t.service` and start the app again. For a development run, export the variable in the shell that starts the service instead:

```sh
export C5T_GITHUB_TOKEN=<token>
```

No scope is needed: the release list of a public repository is readable with any valid token, so a fine-grained token with no permissions works.

### Surfaces and endpoints

- Banner above the navigation and the version badge in the nav: offer, install, dismiss. The badge shows the candidate next to the running version.
- Tray row `Update: …` in the service menu, refreshed by the same 60-second poll as the sync row. It is informational: installing happens in the banner.
- REST: `GET /api/v1/update/status`, `POST /api/v1/update/check` with the body `{"manual": true}` or `{"manual": false}`, `POST /api/v1/update/install`, and `POST /api/v1/update/restart`. Every state change is broadcast on `/ws` as `UpdateStateChanged { status, version }`.

### Signing key (one time)

The updater refuses unsigned artifacts, so `cargo tauri build` signs the updater artifact with a minisign key and stops when the key is missing. Generate the key once:

```sh
cargo tauri signer generate -w ~/.tauri/context.key
```

Paste the contents of `~/.tauri/context.key.pub` into `plugins.updater.pubkey` in both `crates/context-app/tauri.conf.json` and `crates/context-app/service/tauri.conf.json`. The two values must be byte-identical: the GUI bundle embeds the key, and the service verifies downloads against its own baked copy. Both files carry the release public key, which is the public half the keygen produced and is public material. A build signs the updater artifact only while the private key is exported: without `TAURI_SIGNING_PRIVATE_KEY`, and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` for a protected key, `cargo tauri build` stops with a message naming the missing variable. After regenerating the key pair, paste the new public key into both files; an empty key stops the build again until you do.

Store the key as repository secrets for the release workflow. The private key secret carries the key file's contents, and the password secret carries the key's password:

```sh
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/context.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

A passwordless key works without the password secret; a protected key needs both. The release workflow exports both and runs `release-manifest`, so CI publishes the updater assets from the first release that carries the key. A release dispatched with an empty `pubkey` in the confs fails the macOS job while signing the updater artifact.

### Per release

#### Version scheme

Every release is tagged `v<major>.<minor>.<patch>-build.<n>`, with `n` the commit count of the built head, and the app inside reports the same version without the leading `v`. A release cut from `0.8.1` at commit 412 is tagged `v0.8.1-build.412` and reports `0.8.1-build.412`. A stable release carries no suffix: `v0.8.2`.

A numeric build identifier compares by value, so `0.8.1-build.412` ranks above `0.8.1-build.99`. That ordering is what makes the updater deliver builds: the plugin compares the manifest version with the version the app reports, and publication order now agrees with semver order. A short sha compared as ASCII text, so `0.8.0-4610dfd` ranked above `0.8.0-340d170` whatever the publication times, and a newer build of one core could not be delivered.

`n` is the commit count of the built commit:

```sh
git rev-list --count HEAD
```

`release` stamps the version itself. It rewrites `[workspace.package] version` in `Cargo.toml` before `trunk build --release` starts, keeps the stamp for the whole build, and restores `Cargo.toml` and `Cargo.lock` byte-identical when the helper ends: on success, on a failed build and on an interrupt. A version that already carries `-build.<n>` is kept as it is, so the version the release workflow committed, the tag and the version the app reports name one string. The stamped version is also written to `target/release/context-version`, which `release-manifest` reads, so the manifest names the build that was compiled.

To build without the helper, stamp the tree yourself first: set `[workspace.package] version` in `Cargo.toml` to `<major>.<minor>.<patch>-build.<n>`, with `n` from the command above, and revert the file once the build ends.

The release workflow stamps the version in its `prep-release` job, before any build runs, and tags that same version in `finalize-release`:

1. `n` is the commit count of the released head. While the tag `v<base>-build.<n>` already exists, `n` moves up by one, so a second run from one head cannot reuse a version.
2. The job writes `<base>-build.<n>` into `[workspace.package] version` in `Cargo.toml`, refreshes `Cargo.lock` and commits both on the release branch.
3. The build jobs check out that branch, so `cargo tauri build` compiles the stamped version and the running app reports it.
4. `finalize-release` creates the release and its tag `v<base>-build.<n>` before any client can fetch a manifest: the manifest URL embeds the tag.

Existing pre-releases keep their sha tags. `v0.8.0-340d170` and its kin stay published and untouched, and they are never selected again, because semver ranks every `0.8.1` core above `0.8.0`. The first build-numbered release must bump the core to `0.8.1`: `0.8.0-build.1` ranks below a running `0.8.0-<sha>`, since `build` compares below `f0ffee` as ASCII text and a numeric identifier compares below an alphanumeric one. From `0.8.1` on, the core orders first and every build number ranks above every `0.8.0` release, so an install that runs a sha-suffixed `0.8.0` still receives the update.

Export the key's contents before the build, then run the two helpers. The CLI reads the variable as key content, not as a path, and `.env` files do not work: the CLI reads the process environment only.

```sh
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$HOME/.tauri/context.key")"
# the generated key carries a password, so export it too:
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="<the key's password>"

release            # builds, notarizes and staples the dmg
release-manifest   # writes target/release/bundle/macos/latest.json
```

`release-manifest` requires exactly one `target/release/bundle/macos/*.app.tar.gz` with its `.sig`, reads the version from `cargo metadata`, and writes the manifest the plugin fetches:

- `version`: the workspace version of the `context` package.
- `pub_date`: the current UTC time in RFC 3339.
- `platforms`: `darwin-aarch64-app` and `darwin-aarch64`, each with the release-asset URL of the artifact and the contents of its `.sig` file.

Upload `Context.app.tar.gz`, `Context.app.tar.gz.sig`, `latest.json` and the dmg to the GitHub release tagged `v<version>`. The release workflow publishes all four from its artifact run; upload them by hand only for a manual, non-CI release. The manifest URLs embed that tag, so the tag must exist before a client checks.

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
