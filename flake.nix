{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    rust-overlay = {
      inputs = {
        flake-utils.follows = "flake-utils";
        nixpkgs.follows = "nixpkgs";
      };
      url = "github:oxalica/rust-overlay";
    };
  };
  outputs = { self, flake-utils, nixpkgs, rust-overlay, ... } @ inputs:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit overlays system;
          };
          rustPlatform = pkgs.makeRustPlatform {
            cargo = pkgs.rust-bin.stable.latest.minimal;
            rustc = pkgs.rust-bin.stable.latest.minimal;
          };

          src = pkgs.lib.cleanSourceWith {
            src = pkgs.lib.cleanSource ./.;
            filter = name: type:
              let baseName = baseNameOf (toString name);
              in !(baseName == "flake.lock" || pkgs.lib.hasSuffix ".nix" baseName);
          };
        in
        {
          formatter = pkgs.nixpkgs-fmt;

          devShells = rec {
            default = embed-server;

            embed-server = pkgs.mkShell {
              buildInputs = with pkgs; [
                rust-bin.stable.latest.default
              ];
            };
          };

          packages = rec {
            default = embed-server;

            embed-server = rustPlatform.buildRustPackage {
              inherit src;

              name = "embed-server";
              meta = {
                description = "Parse websites and generate previews for use in other websites";
                repository = "https://github.com/Lantern-chat/embed-service";
              };

              cargoBuildFlags = "-p embed-server";

              cargoLock = {
                lockFile = ./Cargo.lock;
                allowBuiltinFetchGit = true;
              };
            };
          };
        }
      );
}
