# Desktop App

The `context` desktop app wraps the REST and MCP server in a native window. The app crate is `crates/context-app` (package `context`, binary `context`). It uses Tauri 2.12 and targets macOS. The CLI binary stays `c5t`.

The app probes `http://127.0.0.1:<port>/health` first. When a server answers with HTTP 200, the app attaches to it. Otherwise the app starts its own server. The port and the SPA origin depend on the build profile:

| Build | Server port | SPA loaded from |
|-------|-------------|-----------------|
| Debug | 3738 | `trunk serve` on `127.0.0.1:8080` |
| Release | 3737 | the in-process server |

A debug build never touches the production port. A debug build also uses its own data directory, `~/.local/share/c5t-dev/`; a release build uses `~/.local/share/c5t/`.

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

The laio `misc` window (`.laio.yaml`) holds two panes side by side: `trunk serve` on the left and `cargo tauri dev` on the right. Run the panes in that order.

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

2. Start the app:

   ```sh
   cargo tauri dev --config '{"identifier":"c5t-dev.kemper.wtf"}'
   ```

   This builds the debug binary and opens the window titled `context`. The app starts the server in-process on `127.0.0.1:3738` and loads the SPA from the Trunk dev server. The server lives and dies with the app, so quitting frees the port. Expected output line:

   ```
   Running `/path/to/context/target/debug/context`
   ```

3. Quit the app from the tray icon. The process stops and frees 3738.

### Dev and production coexistence

The single-instance plugin keys its lock on the app identifier. The identifier `c5t.kemper.wtf` maps to the socket `/tmp/c5t_kemper_wtf_si.sock`; the identifier `c5t-dev.kemper.wtf` maps to `/tmp/c5t_dev_kemper_wtf_si.sock`. Two apps hold separate locks only when their identifiers differ, so the `dev` helper passes the dev identifier and the installed app keeps the production one.

| First app running | Second app started | Result |
|-------------------|--------------------|--------|
| Production | `dev` | Both run. Separate identifiers, separate sockets. |
| `dev` | Production | Both run. Separate identifiers, separate sockets. |
| `dev` | `dev` | The second instance prints `Running .../target/debug/context` and exits within a few seconds. |
| Production | Production | The second instance exits and the first restores its window. |

The `dev` helper runs `cargo tauri dev --config '{"identifier":"c5t-dev.kemper.wtf"}'`. A bare `cargo tauri dev` without the flag still uses `c5t.kemper.wtf`: it keys the production socket and exits when the installed app runs, so start the app through the helper.

### Runtime behavior

The app is a macOS accessory agent. It has no Dock icon and no Cmd+Tab entry, and the menu bar belongs to the frontmost regular app. The window is a transient view of a service that keeps running, because MCP clients keep using the server while the window is closed.

- The close button, Cmd+W, Cmd+Q, and the application menu item Close Window close the window only. The app and its server keep running.
- The menu bar tray icon restores the window: choose **Show context**. Launching the app again does the same through the single-instance handler.
- The tray menu shows the MCP endpoint for the build profile. The sync row appears after `c5t sync init` and reports the working tree state.
- The tray item **Quit** is the only process exit. It stops the server.

### Headless server

To work on the server without the window, stop the app pane, then run the server in a spare pane:

```sh
cargo run --bin c5t api
```

A debug build binds `0.0.0.0:3738` by default and uses the same dev database, `~/.local/share/c5t-dev/context.db`. Run one server per port: when the headless server already runs, the app attaches to it, and quitting the app leaves 3738 taken.

This command fails when 3738 is in use. Expected error:

```
Error:   × Failed to bind to address 0.0.0.0:3738: Address already in use
```

Frontend-only work does not need the app. See [Frontend Architecture](frontend.md).

## Release build

The release build produces `context.app` (executable `Contents/MacOS/context`) and the dmg `context_<version>_aarch64.dmg`. Both land under `target/release/bundle/`.

A release build binds 3737 for its in-process server. The port must be free, or answered by a healthy server the app can attach to. A debug build uses 3738.

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
6. Run `cargo tauri build`.
7. Resolve exactly one `target/release/bundle/dmg/context*_aarch64.dmg`.
8. Run `xcrun notarytool submit <dmg> --key <key path> --key-id <key id> --issuer <issuer id> --wait`.
9. Run `xcrun stapler staple <dmg>`.

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

### The app reports "Failed to start the context server"

The dialog text names the cause, for example:

```
Failed to start the context server: Failed to bind to address 127.0.0.1:3738: Address already in use (os error 48)
```

A debug build binds 3738; a release build binds 3737. The port is held by a process that does not answer `/health` with HTTP 200. Find the process, stop it, and start the app again. To see the same error from the command line, run `cargo run --bin c5t api` while the port is taken; it exits with status 1 and `Failed to bind to address 0.0.0.0:3738: Address already in use`.

### Port 3737 or 3738 is already in use

Find the listener:

```sh
lsof -nP -iTCP:3738 -sTCP:LISTEN
# COMMAND   PID      USER   FD   TYPE ... NODE NAME
# c5t     69904 christian   12u  IPv4 ...  TCP *:3738 (LISTEN)
```

No output means the port is free. Use the same command with 3737 for a release build.

A debug app and the headless dev server share 3738. A release app and a headless `c5t api` share 3737. Run one server per port: quit the extra app from the tray, or stop the extra server. The app attaches to a healthy server instead of starting a second one.

### The window is blank or shows "The frontend assets are not embedded in this build"

The binary embedded the stub `dist/index.html` that `crates/context-server/build.rs` creates for fresh clones. Stop `trunk serve`, run `trunk build --release`, then rebuild the app (`release`, or `cargo tauri build`). A debug build reads `dist/` at runtime, but its window loads the SPA from `trunk serve` on 8080, so keep the dev server running for debug work.

### The release build fails intermittently while `trunk serve` runs

This is the race described in [Stop `trunk serve` first](#stop-trunk-serve-first). Stop the dev server pane and rerun. The `release` helper refuses to start while port 8080 has a listener.

### The dmg Gatekeeper check fails

Use the context form shown in [Check the result](#check-the-result). The bare `spctl --assess --type open <dmg>` form rejects a notarized disk image.
