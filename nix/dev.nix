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
  check = pkgs.writeShellScriptBin "check" ''cargo check'';
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
    trunk build --release
    exec cargo build --release "$@"
  '';
  coverage = pkgs.writeShellScriptBin "coverage" ''
    ${platformArgs}
    exec cargo llvm-cov nextest $PLATFORM_ARGS --html --output-dir coverage/ --ignore-filename-regex "/test/|/test$" "$@"
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
      echo ""
      echo "The app crate 'context' is macOS-only, so tests, clippy, and coverage"
      echo "skip it with --exclude context on other systems."
      echo ""
      echo "Desktop app (macOS only), run from this shell:"
      echo "  cargo tauri dev"
      echo "    Attaches to the running laio panes: it waits for the debug server"
      echo "    on 127.0.0.1:3738 and loads its window from the trunk dev server"
      echo "    on 127.0.0.1:8080. Keep both panes running and start 'trunk serve'"
      echo "    first; the panes and the app run side by side."
      echo "  cargo tauri build"
      echo "    Release bundle. Run 'trunk build --release' first so the bundle"
      echo "    embeds the real SPA instead of the fresh-clone stub."
      echo "  cargo tauri build -- --profile fast-bundle"
      echo "    Fast iteration bundle (no LTO, no size tweaks), artifacts in"
      echo "    target/fast-bundle/bundle/."
    '';
  }
