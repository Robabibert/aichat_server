{
  description = "aichat server";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };
  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
    in {
      packages.${system}.default = pkgs.callPackage ./. { };

    };
}

