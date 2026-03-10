{
  description = "A Nix flake for A Rust-based wallpaper manager that fetches wallpapers from Wallhaven";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      utils,
    }:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "rust-paper";
          version = "0.1.3";
          src = ./.;
          cargoHash = "sha256-RmdQvJ5On/HVvuKun53cNBOcNWwdOo0fGqdMTluJMWY=";
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl ];
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            pkg-config
            openssl
          ];
        };
      }
    );
}
