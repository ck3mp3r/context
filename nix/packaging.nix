{
  inputs,
  system,
  pkgs,
  cargoToml,
  cargoLock,
  overlays,
}: let
  supportedTargets = ["aarch64-darwin" "aarch64-linux" "x86_64-linux"];

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
    x86_64-linux =
      if builtins.pathExists ../data/x86_64-linux.json
      then builtins.fromJSON (builtins.readFile ../data/x86_64-linux.json)
      else {};
  };

  # Stage 1: Build frontend WASM assets with trunk
  wasmToolchain = inputs.fenix.packages.${system}.combine [
    inputs.fenix.packages.${system}.stable.cargo
    inputs.fenix.packages.${system}.stable.rustc
    inputs.fenix.packages.${system}.targets.wasm32-unknown-unknown.stable.rust-std
  ];

  frontendAssets =
    (pkgs.makeRustPlatform {
      cargo = wasmToolchain;
      rustc = wasmToolchain;
    })
    .buildRustPackage {
      pname = "context-frontend";
      inherit (cargoToml.package) version;
      src = ../.;
      inherit cargoLock;

      nativeBuildInputs = with pkgs; [
        trunk
        wasm-bindgen-cli
        nodejs
        nodePackages.tailwindcss
      ];

      buildPhase = ''
        # Set writable HOME for wasm-bindgen cache
        export HOME=$TMPDIR
        trunk build --release
      '';

      installPhase = ''
        cp -r dist $out
      '';

      doCheck = false;
    };

  # Stage 2: Prepare source with pre-built frontend assets
  srcWithFrontend = pkgs.runCommand "context-src-with-frontend" {} ''
    cp -r ${../.} $out
    chmod -R +w $out
    cp -r ${frontendAssets} $out/dist
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
    fenix = inputs.fenix;
    nixpkgs = inputs.nixpkgs;
    src = srcWithFrontend;
    packageName = "context";
    archiveAndHash = false;
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
    fenix = inputs.fenix;
    nixpkgs = inputs.nixpkgs;
    src = srcWithFrontend;
    packageName = "archive";
    archiveAndHash = true;
  };

  # Import container image build
  containerImage = import ./container.nix {
    inherit pkgs cargoToml;
    defaultPackage = regularPackages.default;
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
