{
  pkgs ? import <nixpkgs> { },
}:

pkgs.rustPlatform.buildRustPackage {
  pname = "rust-paper";
  version = "0.1.3";

  src = pkgs.fetchFromGitHub {
    owner = "abhaythakur71181";
    repo = "rust-paper";
    rev = "main";
    sha256 = "sha256-RmdQvJ5On/HVvuKun53cNBOcNWwdOo0fGqdMTluJMWY=";
  };
  cargoHash = "sha256-RmdQvJ5On/HVvuKun53cNBOcNWwdOo0fGqdMTluJMWY=";
}
