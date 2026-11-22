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

    # a live trunk serve rewrites dist/ while rust-embed reads it at compile time
    if lsof -nP -iTCP:8080 -sTCP:LISTEN >/dev/null 2>&1; then
      echo "release: port 8080 is in use by a running 'trunk serve'" >&2
      echo "release: trunk serve rewrites dist/ while rust-embed reads it at compile" >&2
      echo "release: time, so the bundle can embed a half-written SPA. Stop the trunk" >&2
      echo "release: pane, then rerun release." >&2
      exit 1
    fi

    trunk build --release
    cargo tauri build

    # resolve the dmg before submitting anything to the notary service
    shopt -s nullglob
    matches=(target/release/bundle/dmg/context*_aarch64.dmg)
    if [ "''${#matches[@]}" -ne 1 ]; then
      echo "release: expected exactly one dmg matching target/release/bundle/dmg/context*_aarch64.dmg, found ''${#matches[@]}:" >&2
      printf '  %s\n' "''${matches[@]}" >&2
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

      # Development scripts
      check
      fmt
      tests
      clippy
      build
      coverage
      release
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
      echo "  release  - Build, notarize and staple the dmg (needs APPLE_API_* credentials)"
      echo ""
      echo "The app crate 'context' is macOS-only, so tests, check, clippy,"
      echo "build, and coverage skip it with --exclude context on other systems."
      echo ""
      echo "Desktop app (macOS only), run from this shell:"
      echo "  cargo tauri dev"
      echo "    Runs as a misc-window pane beside 'trunk serve'. The app starts the"
      echo "    REST and MCP server in-process on 127.0.0.1:3738 and loads the SPA"
      echo "    from the trunk dev server on 127.0.0.1:8080. The server lives and"
      echo "    dies with the app, so quitting the app frees 3738."
      echo "    Headless server work: stop the app pane, then run"
      echo "    'cargo run --bin c5t api' in a spare pane — same port, same dev"
      echo "    database. Run one or the other: if both run, the app attaches to"
      echo "    the headless server and quitting the app leaves 3738 taken."
      echo "  cargo tauri build"
      echo "    Release bundle. Run 'trunk build --release' first so the bundle"
      echo "    embeds the real SPA instead of the fresh-clone stub."
      echo "  cargo tauri build -- --profile fast-bundle"
      echo "    Fast iteration bundle (no LTO, no size tweaks), artifacts in"
      echo "    target/fast-bundle/bundle/."
    '';
  }
