# Development shell for c5t
{
  pkgs,
  inputs,
  system,
}: let
  toolchain = inputs.rustnix.lib.rust.mkToolchain {
    inherit system;
    targets = ["wasm32-unknown-unknown"];
    extras = ["rustfmt" "clippy" "rust-analyzer" "llvm-tools-preview"];
  };

  # LLVM tools for cargo-llvm-cov; must match the LLVM version used by rustc
  llvmPkgs = pkgs.llvmPackages_22;

  # The `context` app crate is macOS-only: its tauri dependency tree needs
  # WebKitGTK and GTK on Linux. Workspace-wide commands skip it everywhere
  # except Darwin, where it takes part in the workspace run.
  platformArgs = ''
    case "$(uname -s)" in
      Darwin) PLATFORM_ARGS="" ;;
      *) PLATFORM_ARGS="--workspace --exclude context" ;;
    esac
  '';

  # Development helper scripts
  check = pkgs.writeShellScriptBin "check" ''
    ${platformArgs}
    exec cargo check $PLATFORM_ARGS "$@"
  '';
  fmt = pkgs.writeShellScriptBin "fmt" ''cargo fmt'';
  tests = pkgs.writeShellScriptBin "tests" ''
    ${platformArgs}
    exec cargo nextest run $PLATFORM_ARGS "$@"
  '';
  clippy = pkgs.writeShellScriptBin "clippy" ''
    ${platformArgs}
    exec cargo clippy --all-targets $PLATFORM_ARGS "$@"
  '';
  # rust-embed bakes `dist/` in at compile time, and the folder is gitignored.
  # The frontend must be built first so a release bundle never ships the stub.
  build = pkgs.writeShellScriptBin "build" ''
    set -euo pipefail
    ${platformArgs}
    trunk build --release
    exec cargo build --release $PLATFORM_ARGS "$@"
  '';
  coverage = pkgs.writeShellScriptBin "coverage" ''
    ${platformArgs}
    exec cargo llvm-cov nextest $PLATFORM_ARGS --html --output-dir coverage/ --ignore-filename-regex "/test/|/test$" "$@"
  '';
  # `cargo tauri build` notarizes and staples the .app, but its dmg step never
  # passes `--notarize` to the bundler's dmg script, so the dmg ships unsigned
  # by the notary service. Submit and staple it here: one command, one release.
  release = pkgs.writeShellScriptBin "release" ''
    set -euo pipefail

    # the app crate and the Apple toolchain are macOS-only: fail before any build
    if [ "$(uname -s)" != "Darwin" ]; then
      echo "release: macOS only — signing and notarization need the Apple toolchain" >&2
      exit 1
    fi

    # credentials first: never start a long build that cannot be notarized
    missing=()
    for name in APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH; do
      if [ -z "''${!name:-}" ]; then
        missing+=("$name")
      fi
    done
    if [ "''${#missing[@]}" -gt 0 ]; then
      echo "release: missing notarization credentials: ''${missing[*]}" >&2
      echo "release: export APPLE_API_KEY, APPLE_API_ISSUER and APPLE_API_KEY_PATH first" >&2
      exit 1
    fi

    # the 8080 preflight below is only meaningful when lsof exists: fail closed
    if ! command -v lsof >/dev/null 2>&1; then
      echo "release: lsof is required but not found in PATH" >&2
      exit 1
    fi

    # a live trunk serve rewrites dist/ while rust-embed reads it at compile time
    if lsof -nP -iTCP:8080 -sTCP:LISTEN >/dev/null 2>&1; then
      echo "release: port 8080 is in use by a running 'trunk serve'" >&2
      echo "release: trunk serve rewrites dist/ while rust-embed reads it at compile" >&2
      echo "release: time, so the bundle can embed a half-written SPA. Stop the trunk" >&2
      echo "release: pane, then rerun release." >&2
      exit 1
    fi

    # The workspace version stamps the build: the tag, the version the app reports
    # and the updater manifest must name one string. A version the release prep
    # already stamped is kept — the branch under build carries the stamp its tag
    # was cut from — and only a bare version is stamped here, with n the commit
    # count of the built head. Semver orders a numeric build identifier by value,
    # so a later release always ranks above an earlier one.
    cargo_toml=Cargo.toml
    if [ ! -f "$cargo_toml" ] || [ ! -f Cargo.lock ]; then
      echo "release: run from the repository root — Cargo.toml and Cargo.lock must be here" >&2
      exit 1
    fi
    stamped_version="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' "$cargo_toml")"
    if [ -z "$stamped_version" ]; then
      echo "release: cannot read [workspace.package] version from $cargo_toml" >&2
      exit 1
    fi

    # restore_workspace returns Cargo.toml and Cargo.lock byte-identical on every
    # exit path, including a failed build, Ctrl-C and a CI cancellation
    backup_dir="$(mktemp -d)"
    cp -p "$cargo_toml" Cargo.lock "$backup_dir/"
    restore_workspace() {
      cp -p "$backup_dir/Cargo.toml" "$cargo_toml"
      cp -p "$backup_dir/Cargo.lock" Cargo.lock
      rm -rf "$backup_dir"
    }
    trap restore_workspace EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM

    case "$stamped_version" in
      *-build.[0-9]*)
        echo "release: version $stamped_version (stamped by the release prep)"
        ;;
      *)
        core="''${stamped_version%%-*}"
        if ! count="$(git rev-list --count HEAD 2>&1)"; then
          echo "release: git rev-list --count HEAD failed: $count" >&2
          exit 1
        fi
        count="''${count//[[:space:]]/}"
        case "$count" in
          ""|*[!0-9]*|0)
            echo "release: git rev-list --count HEAD returned '$count', not a commit count" >&2
            exit 1
            ;;
        esac
        stamped_version="$core-build.$count"
        awk -v version="$stamped_version" '
          /^\[workspace\.package\]/ { in_package = 1 }
          in_package && !stamped && /^version *=/ { sub(/"[^"]*"/, "\"" version "\""); stamped = 1 }
          { print }
        ' "$cargo_toml" > "$backup_dir/stamped.toml"
        cat "$backup_dir/stamped.toml" > "$cargo_toml"
        echo "release: version $stamped_version (stamped from $count commits)"
        ;;
    esac

    trunk build --release
    # the bundler copies the sidecar into Contents/MacOS and signs it with the app,
    # so it must exist before the build: a missing path fails the bundle. The
    # suffix is the target triple the bundler appends for lookup and strips at
    # the destination.
    cargo build --release -p context-cli
    cp target/release/c5t target/release/c5t-aarch64-apple-darwin
    cargo tauri build

    # record what was built: release-manifest reads it once the stamp is gone, so
    # the manifest names the version the app reports. Then put the tree back to the
    # version the commit names before the dmg work starts.
    printf '%s\n' "$stamped_version" > target/release/context-version
    restore_workspace
    trap - EXIT INT TERM

    # resolve the dmg before submitting anything to the notary service
    shopt -s nullglob
    matches=(target/release/bundle/dmg/Context*_aarch64.dmg)
    if [ "''${#matches[@]}" -ne 1 ]; then
      echo "release: expected exactly one dmg matching target/release/bundle/dmg/Context*_aarch64.dmg, found ''${#matches[@]}:" >&2
      if [ "''${#matches[@]}" -gt 0 ]; then
        printf '  %s\n' "''${matches[@]}" >&2
      fi
      exit 1
    fi
    dmg="''${matches[0]}"

    xcrun notarytool submit "$dmg" \
      --key "$APPLE_API_KEY_PATH" \
      --key-id "$APPLE_API_KEY" \
      --issuer "$APPLE_API_ISSUER" \
      --wait
    xcrun stapler staple "$dmg"

    echo ""
    echo "dmg: $dmg"
    echo "Verify:"
    echo "  xcrun stapler validate $dmg"
    echo "  spctl --assess --type open --verbose --context context:primary-signature $dmg"
  '';
  # The updater reads a manifest asset from each release: one JSON file naming
  # the signed artifact of that version. Assemble it from what `release` built.
  release-manifest = pkgs.writeShellScriptBin "release-manifest" ''
    set -euo pipefail

    # the updater artifacts are macOS bundles: fail before reading anything
    if [ "$(uname -s)" != "Darwin" ]; then
      echo "release-manifest: macOS only — the updater artifacts are macOS bundles" >&2
      exit 1
    fi

    # resolve the artifact before writing anything: the bundler emits one bundle
    shopt -s nullglob
    matches=(target/release/bundle/macos/*.app.tar.gz)
    if [ "''${#matches[@]}" -ne 1 ]; then
      echo "release-manifest: expected exactly one artifact matching target/release/bundle/macos/*.app.tar.gz, found ''${#matches[@]}:" >&2
      if [ "''${#matches[@]}" -gt 0 ]; then
        printf '  %s\n' "''${matches[@]}" >&2
      fi
      echo "release-manifest: run 'release' first; it needs the updater signing key" >&2
      exit 1
    fi
    artifact="''${matches[0]}"
    signature_file="$artifact.sig"
    if [ ! -f "$signature_file" ]; then
      echo "release-manifest: missing $signature_file — the build did not sign the artifact" >&2
      exit 1
    fi

    # release restores Cargo.toml once the build ends, so the version it compiled
    # lives in target/release/context-version; a hand-stamped tree still reads
    # cargo metadata, which names the same version
    version_file=target/release/context-version
    if [ -f "$version_file" ]; then
      version="$(cat "$version_file")"
    else
      version="$(cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | select(.name == "context") | .version')"
    fi
    if [ -z "$version" ] || [ "$version" = "null" ]; then
      echo "release-manifest: cannot read the version of the 'context' package" >&2
      echo "release-manifest: run 'release' first; it records the version it built" >&2
      exit 1
    fi

    # the manifest names the release asset, so the tag is part of the URL
    tag="v$version"
    name="$(basename "$artifact")"
    url="https://github.com/ck3mp3r/context/releases/download/$tag/$name"
    signature="$(cat "$signature_file")"
    pub_date="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    manifest="target/release/bundle/macos/latest.json"

    jq -n \
      --arg version "$version" \
      --arg pub_date "$pub_date" \
      --arg url "$url" \
      --arg signature "$signature" \
      '{
        version: $version,
        pub_date: $pub_date,
        platforms: {
          "darwin-aarch64-app": { url: $url, signature: $signature },
          "darwin-aarch64": { url: $url, signature: $signature }
        }
      }' > "$manifest"

    echo ""
    echo "manifest: $manifest"
    echo "Upload to the $tag release:"
    echo "  $name"
    echo "  $name.sig"
    echo "  latest.json"
  '';
  # The window loads the SPA from the trunk dev server, so a missing 8080 is a
  # dead window: refuse before cargo builds anything. Both 3738 states are
  # valid, so that probe reports the mode instead of refusing.
  dev = pkgs.writeShellScriptBin "dev" ''
    set -euo pipefail

    # the app crate and the Apple toolchain are macOS-only
    if [ "$(uname -s)" != "Darwin" ]; then
      echo "dev: macOS only — the app crate needs the Apple toolchain" >&2
      exit 1
    fi

    # the port probes below are only meaningful when lsof exists: fail closed
    if ! command -v lsof >/dev/null 2>&1; then
      echo "dev: lsof is required but not found in PATH" >&2
      exit 1
    fi

    # the window loads the SPA from the trunk dev server: no trunk, dead window
    if ! lsof -nP -iTCP:8080 -sTCP:LISTEN >/dev/null 2>&1; then
      echo "dev: nothing serves 8080 — run 'serve' first" >&2
      echo "dev: the app window loads its SPA from the trunk dev server" >&2
      exit 1
    fi

    # the GUI only attaches to a service: start 'dev-service' in its own pane
    if ! lsof -nP -iTCP:3738 -sTCP:LISTEN >/dev/null 2>&1; then
      echo "dev: nothing serves 3738 — start 'dev-service' first" >&2
      echo "dev: the GUI spawns the service only outside the dev loop" >&2
      exit 1
    fi
    echo "dev: 3738 is served — the GUI attaches to that service"

    # the identifier override splits the single-instance lock from production
    exec cargo tauri dev --config '{"identifier":"c5t-dev.kemper.wtf"}'
  '';
  # The service owns the tray and the API server; the GUI is only a view over it.
  # Both 3738 states are valid here: the service attaches or starts the server.
  dev-service = pkgs.writeShellScriptBin "dev-service" ''
    set -euo pipefail

    # the app crate and the Apple toolchain are macOS-only
    if [ "$(uname -s)" != "Darwin" ]; then
      echo "dev-service: macOS only — the app crate needs the Apple toolchain" >&2
      exit 1
    fi

    # the port probe below is only meaningful when lsof exists: fail closed
    if ! command -v lsof >/dev/null 2>&1; then
      echo "dev-service: lsof is required but not found in PATH" >&2
      exit 1
    fi

    # both 3738 states work: say which one this launch gets
    if lsof -nP -iTCP:3738 -sTCP:LISTEN >/dev/null 2>&1; then
      echo "dev-service: 3738 is held — the service attaches to it; it does not own the server"
    else
      echo "dev-service: the service starts the server on 3738; it dies with the service"
    fi

    # TAURI_CONFIG bakes the identifier, so this value must differ from 'dev'
    exec env TAURI_CONFIG='{"identifier":"c5t-dev.service.kemper.wtf"}' cargo run --bin context-service
  '';
  # The app window loads the SPA from here, so refuse a second instance before
  # trunk fails on the busy port.
  serve = pkgs.writeShellScriptBin "serve" ''
    set -euo pipefail

    # the port probe below is only meaningful when lsof exists: fail closed
    if ! command -v lsof >/dev/null 2>&1; then
      echo "serve: lsof is required but not found in PATH" >&2
      exit 1
    fi

    if lsof -nP -iTCP:8080 -sTCP:LISTEN >/dev/null 2>&1; then
      echo "serve: 8080 is already served — a 'trunk serve' is already running" >&2
      echo "serve: use that one, or stop it first" >&2
      exit 1
    fi

    exec trunk serve
  '';
  # The app and the headless server share 3738: run one or the other, so
  # refuse when the port is taken instead of racing the app.
  api = pkgs.writeShellScriptBin "api" ''
    set -euo pipefail

    # the port probe below is only meaningful when lsof exists: fail closed
    if ! command -v lsof >/dev/null 2>&1; then
      echo "api: lsof is required but not found in PATH" >&2
      exit 1
    fi

    if lsof -nP -iTCP:3738 -sTCP:LISTEN >/dev/null 2>&1; then
      echo "api: 3738 is already in use — run one or the other" >&2
      echo "api: stop the app (it holds 3738) or the headless server first" >&2
      exit 1
    fi

    exec cargo run --bin c5t api
  '';
in
  pkgs.mkShellNoCC {
    name = "context-dev";

    # cargo-llvm-cov locates the LLVM binaries through these variables
    LLVM_COV = "${llvmPkgs.llvm}/bin/llvm-cov";
    LLVM_PROFDATA = "${llvmPkgs.llvm}/bin/llvm-profdata";

    buildInputs = [
      toolchain
      pkgs.prek
      # cargo-llvm-cov's own test suite segfaults instrumented binaries inside
      # the nix sandbox (nixpkgs cargo-llvm-cov, upstream not fixed yet),
      # which fails the whole dev shell build. Skip its self-test.
      (pkgs.cargo-llvm-cov.overrideAttrs (_: {doCheck = false;}))
      pkgs.cargo-nextest
      # tauri-cli tracks the `tauri` crate 2.12.x used by the app crate
      pkgs.cargo-tauri
      pkgs.trunk
      (pkgs.callPackage ./wasm-bindgen-cli-bin.nix {})
      pkgs.tailwindcss_4
      pkgs.act
      pkgs.jq

      # Development scripts
      check
      fmt
      tests
      clippy
      build
      coverage
      release
      release-manifest
      dev
      dev-service
      serve
      api
    ];

    shellHook = ''
      prek install -f --hook-type pre-commit --hook-type pre-push
      echo "c5t development shell"
      echo "Rust: $(rustc --version)"
      echo "cargo-llvm-cov: $(cargo llvm-cov --version)"
      echo "cargo-nextest: $(cargo nextest --version)"
      echo "cargo-tauri: $(cargo tauri --version)"
      echo ""
      echo "Available commands:"
      echo "  check    - Run cargo check"
      echo "  fmt      - Run cargo fmt"
      echo "  tests    - Run tests (cargo nextest run)"
      echo "  clippy   - Run cargo clippy"
      echo "  build    - Build release binary (trunk build --release, then cargo build --release)"
      echo "  coverage - Run workspace coverage (HTML report in coverage/)"
      echo "  release  - Stamp the build version, build, notarize and staple the dmg (needs APPLE_API_* credentials)"
      echo "  release-manifest - Generate latest.json from the built updater artifacts (run after release)"
      echo "  dev      - Start the GUI as c5t-dev.kemper.wtf (needs 'dev-service' on 3738)"
      echo "  dev-service - Start the service: tray + API on 3738, c5t-dev.service.kemper.wtf"
      echo "  serve    - Start the Trunk dev server on 8080"
      echo "  api      - Start the headless REST and MCP server on 3738"
      echo ""
      echo "The app crate 'context' is macOS-only, so tests, check, clippy,"
      echo "build, and coverage skip it with --exclude context on other systems."
      echo ""
      echo "Desktop app (macOS only), run from this shell:"
      echo "  serve"
      echo "    Starts the Trunk dev server on 127.0.0.1:8080. Run it in one pane;"
      echo "    the GUI window loads the SPA from there, so start it first."
      echo "  dev-service"
      echo "    Starts the service in a second pane: it owns the tray and the REST"
      echo "    and MCP server on 127.0.0.1:3738. With 3738 free it starts the"
      echo "    server; when a server already answers, it attaches to that one."
      echo "    It runs as c5t-dev.service.kemper.wtf with its own single-instance"
      echo "    socket, so the service, the GUI and the installed app coexist."
      echo "  dev"
      echo "    Starts the GUI in a third pane. The GUI is a view over the service:"
      echo "    it attaches to 3738 and needs 'dev-service' running. Cmd+Q quits"
      echo "    the GUI only; the service tray Quit stops the whole pair."
      echo "    It runs as c5t-dev.kemper.wtf, so it and the installed app coexist."
      echo "  api"
      echo "    Starts the headless REST and MCP server on 127.0.0.1:3738 from a"
      echo "    debug build — same port, same dev database. Headless server work:"
      echo "    stop the service pane, then run this in a spare pane. Run one or"
      echo "    the other: if both run, the service attaches to the headless server"
      echo "    and quitting the service leaves 3738 taken."
      echo "  cargo tauri build"
      echo "    Release bundle. Run 'trunk build --release' first so the bundle"
      echo "    embeds the real SPA instead of the fresh-clone stub."
      echo "  cargo tauri build -- --profile fast-bundle"
      echo "    Fast iteration bundle (no LTO, no size tweaks), artifacts in"
      echo "    target/fast-bundle/bundle/."
    '';
  }
