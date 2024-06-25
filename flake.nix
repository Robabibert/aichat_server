{
  description = "aichat server";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  outputs = { self, nixpkgs, rust-overlay }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };
      toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

    in {
      packages.${system}.default = pkgs.callPackage ./. { };
      devShells.${system}.default = pkgs.mkShell {
        packages = [
          pkgs.cargo-make
          pkgs.libudev-zero
          pkgs.nodePackages.bash-language-server
          pkgs.nodePackages.yaml-language-server
          pkgs.openssl
          pkgs.pkg-config
          pkgs.rust-analyzer-unwrapped
          pkgs.taplo
          pkgs.sccache

          toolchain
        ];
        RUSTC_WRAPPER = "sccache";
        RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/src";
        RUST_BACKTRACE = 1;
        PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        LD_LIBRARY_PATH =
          "${pkgs.lib.makeLibraryPath [ pkgs.alsaLib pkgs.udev ]}";
        shellHook = "exec fish";
      };

    };
}
