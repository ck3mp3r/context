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

  # Development helper scripts
  check = pkgs.writeShellScriptBin "check" ''cargo check'';
  fmt = pkgs.writeShellScriptBin "fmt" ''cargo fmt'';
  tests = pkgs.writeShellScriptBin "tests" ''cargo nextest run'';
  clippy = pkgs.writeShellScriptBin "clippy" ''cargo clippy'';
  build = pkgs.writeShellScriptBin "build" ''cargo build --release'';
  coverage = pkgs.writeShellScriptBin "coverage" ''cargo llvm-cov nextest --workspace --html --output-dir coverage/ --ignore-filename-regex "/test/|/test$"'';
in
  pkgs.mkShellNoCC {
    name = "context-dev";

    # cargo-llvm-cov locates the LLVM binaries through these variables
    LLVM_COV = "${llvmPkgs.llvm}/bin/llvm-cov";
    LLVM_PROFDATA = "${llvmPkgs.llvm}/bin/llvm-profdata";

    buildInputs = [
      toolchain
      pkgs.prek
      pkgs.cargo-llvm-cov
      pkgs.cargo-nextest
      pkgs.trunk
      (pkgs.callPackage ./wasm-bindgen-cli.nix {})
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
      echo ""
      echo "Available commands:"
      echo "  check    - Run cargo check"
      echo "  fmt      - Run cargo fmt"
      echo "  tests    - Run tests (cargo nextest run)"
      echo "  clippy   - Run cargo clippy"
      echo "  build    - Build release binary"
      echo "  coverage - Run workspace coverage (HTML report in coverage/)"
    '';
  }
