{
  description = "Context - Rust CLI and API";

  inputs = {
    nixpkgs.url = "github:NixOs/nixpkgs/nixpkgs-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    rustnix = {
      url = "github:ck3mp3r/flakes?dir=rustnix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-parts.follows = "flake-parts";
    };
  };

  outputs = inputs @ {
    self,
    flake-parts,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["aarch64-darwin" "aarch64-linux" "x86_64-darwin" "x86_64-linux"];
      perSystem = {system, ...}: let
        overlays = [
          inputs.rustnix.lib.rust.overlays.fenix
        ];
        pkgs = import inputs.nixpkgs {inherit system overlays;};

        cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
        cargoLock = {
          lockFile = ./Cargo.lock;
          outputHashes = {
            "tree-sitter-nu-0.0.1" = "sha256-OL3fqHjimJ9VrR2UoeIdLxKKcsA1J80A9T8GSBO9KwE=";
            "tree-sitter-kotlin-0.4.0" = "sha256-sfaLNslFpW/NiFxJNXqoMEykBdfjoxgGBbfntrszaUo=";
            "tree-sitter-typescript-0.23.2" = "sha256-A0M6IBoY87ekSV4DfGHDU5zzFWdLjGqSyVr6VENgA+s=";
          };
        };

        # Import packaging logic
        packaging = import ./nix/packaging.nix {
          inherit
            inputs
            system
            pkgs
            cargoToml
            cargoLock
            overlays
            ;
        };
      in {
        inherit (packaging) apps packages;

        devShells = {
          default = import ./nix/dev.nix {
            inherit pkgs inputs system;
          };

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
