{
  pkgs,
  lib,
  config,
  inputs,
  ...
}: {
  packages = let
    toolchain = inputs.fenix.packages.${pkgs.system}.combine [
      inputs.fenix.packages.${pkgs.system}.stable.cargo
      inputs.fenix.packages.${pkgs.system}.stable.rustc
      inputs.fenix.packages.${pkgs.system}.stable.rust-analyzer
      inputs.fenix.packages.${pkgs.system}.stable.llvm-tools-preview
      inputs.fenix.packages.${pkgs.system}.targets.wasm32-unknown-unknown.stable.rust-std
    ];
  in [
    toolchain
    pkgs.cargo-tarpaulin
    pkgs.cargo-llvm-cov
    pkgs.trunk
    pkgs.wasm-bindgen-cli
    pkgs.nodejs
    pkgs.tailwindcss_4
    pkgs.act # GitHub Actions local testing
  ];

  scripts = {
    check = {
      exec = "cargo check";
      description = "Run cargo check";
    };
    fmt = {
      exec = "cargo fmt";
      description = "Run cargo fmt";
    };
    tests = {
      exec = "cargo test";
      description = "Run cargo test";
    };
    clippy = {
      exec = "cargo clippy $@";
      description = "Run cargo clippy";
    };
    coverage = {
      exec = "cargo llvm-cov --html --open";
      description = "Generate code coverage report with cargo-llvm-cov";
    };
    build = {
      exec = "cargo build --release";
      description = "Build release binary";
    };
    build-container = {
      exec = ''
        docker run --rm \
          -v $(pwd):/workspace \
          -w /workspace \
          nixos/nix:latest bash -c \
          "git config --global --add safe.directory /workspace && \
           nix --extra-experimental-features 'nix-command flakes' build \
           .#container --system x86_64-linux --impure && cat result" | docker load
      '';
      description = "Build ARM64 container image using Nix in Docker";
    };
    test-container-workflow = {
      exec = ''
        act workflow_dispatch \
          -W .github/workflows/container-build.yaml \
          --container-architecture linux/arm64 \
          --container-daemon-socket /var/run/docker.sock \
          --privileged \
          --secret GITHUB_TOKEN \
          --input push_latest=false \
          -P ubuntu-latest=catthehacker/ubuntu:js-latest \
          -P ubuntu-24.04-arm=catthehacker/ubuntu:js-latest \
          "$@"
      '';
      description = "Test container-build workflow with act (pass -n for dry-run)";
    };
  };

  git-hooks.hooks = {
    rustfmt = {
      enable = true;
      packageOverrides.rustfmt = inputs.fenix.packages.${pkgs.system}.stable.rustfmt;
    };
    clippy = {
      enable = true;
      packageOverrides.clippy = inputs.fenix.packages.${pkgs.system}.stable.clippy;
    };
    # Custom pre-push hook to run tests
    test-on-push = {
      enable = true;
      name = "Run tests";
      entry = "cargo test";
      language = "system";
      stages = ["pre-push"];
      pass_filenames = false;
    };
  };

  enterShell = let
    scriptLines =
      lib.mapAttrsToList (
        name: script: "printf '  %-10s  %s\\n' '${name}' '${script.description}'"
      )
      config.scripts;
  in ''
    echo
    echo "Helper scripts you can run to make your development richer:"
    echo ""
    ${lib.concatStringsSep "\n" scriptLines}
    echo
  '';
}
