{
  inputs,
  system,
  pkgs,
  cargoToml,
  cargoLock,
  overlays,
}: let
  supportedTargets = ["aarch64-darwin" "aarch64-linux" "x86_64-darwin" "x86_64-linux"];

  # Install data for pre-built releases (will be generated during release)
  installData = {
    aarch64-darwin =
      if builtins.pathExists ../data/aarch64-darwin.json
      then builtins.fromJSON (builtins.readFile ../data/aarch64-darwin.json)
      else {};
    aarch64-linux =
      if builtins.pathExists ../data/aarch64-linux.json
      then builtins.fromJSON (builtins.readFile ../data/aarch64-linux.json)
      else {};
    x86_64-darwin =
      if builtins.pathExists ../data/x86_64-darwin.json
      then builtins.fromJSON (builtins.readFile ../data/x86_64-darwin.json)
      else {};
    x86_64-linux =
      if builtins.pathExists ../data/x86_64-linux.json
      then builtins.fromJSON (builtins.readFile ../data/x86_64-linux.json)
      else {};
  };

  # Stage 1: Build frontend WASM assets with trunk
  frontendAssets =
    (inputs.rustnix.lib.rust.mkRustPlatform {
      inherit system overlays;
      nixpkgs = inputs.nixpkgs;
      targets = ["wasm32-unknown-unknown"];
    })
    .buildRustPackage {
      pname = "context-frontend";
      version = cargoToml.workspace.package.version;
      src = ../.;
      # context-frontend is excluded from the workspace and has its own
      # Cargo.lock (root lock omits its deps like codee/leptos).
      cargoRoot = "crates/context-frontend";
      cargoLock = {
        lockFile = ../crates/context-frontend/Cargo.lock;
      };

      nativeBuildInputs = [
        pkgs.trunk
        (pkgs.callPackage ./wasm-bindgen-cli.nix {})
        pkgs.tailwindcss_4
      ];

      buildPhase = ''
        # Set writable HOME for wasm-bindgen cache
        export HOME=$TMPDIR
        trunk build --release
      '';

      installPhase = ''
        cp -r crates/context-server/dist $out
      '';

      doCheck = false;
    };

  # Stage 2: Prepare source with pre-built frontend assets
  srcWithFrontend = pkgs.runCommand "context-src-with-frontend" {} ''
    cp -r ${../.} $out
    chmod -R +w $out
    cp -r ${frontendAssets} $out/crates/context-server/dist
  '';

  # Build regular packages (no archives)
  regularPackages = inputs.rustnix.lib.rust.buildTargetOutputs {
    inherit
      cargoToml
      cargoLock
      overlays
      pkgs
      system
      installData
      supportedTargets
      ;
    nixpkgs = inputs.nixpkgs;
    src = srcWithFrontend;
    packageName = "context";
    workspaceMember = "context-cli";
    archiveAndHash = false;
    nativeBuildInputs = [];
    extraArgs = {
      buildFeatures = ["context-server/embed-frontend"];
    };
  };

  # Build archive packages (creates archive with system name)
  archivePackages = inputs.rustnix.lib.rust.buildTargetOutputs {
    inherit
      cargoToml
      cargoLock
      overlays
      pkgs
      system
      installData
      supportedTargets
      ;
    nixpkgs = inputs.nixpkgs;
    src = srcWithFrontend;
    packageName = "archive";
    workspaceMember = "context-cli";
    archiveAndHash = true;
    nativeBuildInputs = [];
    extraArgs = {
      buildFeatures = ["context-server/embed-frontend"];
    };
  };

  # Custom minimal git for container
  gitCustom = pkgs.callPackage ./git-minimal.nix {};

  # Import container image build
  containerImage = import ./container.nix {
    inherit pkgs cargoToml;
    defaultPackage = regularPackages.context;
    git = gitCustom;
  };
  # Check if we're on Darwin (macOS)
  isDarwin = builtins.match ".*-darwin" system != null;
in {
  # Export all package outputs
  packages =
    regularPackages
    // archivePackages
    // (
      # Only include container on non-Darwin systems (Linux)
      if isDarwin
      then {}
      else {container = containerImage;}
    );

  # Export apps
  apps = {
    default = {
      type = "app";
      program = "${regularPackages.default}/bin/c5t";
    };
  };
}
