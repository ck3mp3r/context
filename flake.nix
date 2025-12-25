{
  description = "Context - Rust CLI and API";

  inputs = {
    nixpkgs.url = "github:NixOs/nixpkgs/nixpkgs-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    devenv = {
      url = "github:cachix/devenv";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rustnix = {
      url = "github:ck3mp3r/flakes?dir=rustnix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
    };
  };

  outputs = inputs @ {
    self,
    flake-parts,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["aarch64-darwin" "aarch64-linux" "x86_64-linux"];
      perSystem = {
        config,
        system,
        ...
      }: let
        supportedTargets = ["aarch64-darwin" "aarch64-linux" "x86_64-linux"];
        overlays = [
          inputs.fenix.overlays.default
        ];
        pkgs = import inputs.nixpkgs {inherit system overlays;};

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        cargoLock = {lockFile = ./Cargo.lock;};

        # Install data for pre-built releases (will be generated during release)
        installData = {
          aarch64-darwin = if builtins.pathExists ./data/aarch64-darwin.json
            then builtins.fromJSON (builtins.readFile ./data/aarch64-darwin.json)
            else {};
          aarch64-linux = if builtins.pathExists ./data/aarch64-linux.json
            then builtins.fromJSON (builtins.readFile ./data/aarch64-linux.json)
            else {};
          x86_64-linux = if builtins.pathExists ./data/x86_64-linux.json
            then builtins.fromJSON (builtins.readFile ./data/x86_64-linux.json)
            else {};
        };

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
          src = ./.;
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
          src = ./.;
          packageName = "archive";
          archiveAndHash = true;
        };
      in {
        apps = {
          default = {
            type = "app";
            program = "${config.packages.default}/bin/c5t";
          };
          api = {
            type = "app";
            program = "${config.packages.default}/bin/c5t-api";
          };
        };

        packages = regularPackages // archivePackages;

        devShells = {
          default = inputs.devenv.lib.mkShell {
            inherit inputs pkgs;
            modules = [
              ./nix/devenv.nix
            ];
          };

          # Classic shell for CI - just toolchains, no devenv
          ci = import ./nix/ci.nix {
            inherit pkgs inputs system;
          };
        };

        formatter = pkgs.alejandra;
      };

      flake = {
        overlays.default = final: prev: {
          context = self.packages.default;
        };
      };
    };
}
