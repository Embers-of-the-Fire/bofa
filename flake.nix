{
  description = "Rust development environment with rust-analyzer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSystem =
        f:
        builtins.listToAttrs (
          map (system: {
            name = system;
            value = f {
              pkgs = import nixpkgs {
                inherit system;
                overlays = [ rust-overlay.overlays.default ];
              };
            };
          }) systems
        );
    in
    {
      devShells = forEachSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = [
              (pkgs.rust-bin.stable.latest.default.override {
                extensions = [
                  "rust-src"
                  "rust-analyzer"
                  "clippy"
                  "rustfmt"
                ];
              })
              pkgs.gcc
              pkgs.pkg-config
              pkgs.openssl
              pkgs.lld
            ];

            env = {
              RUST_BACKTRACE = "1";
              RUSTFLAGS = "-C link-arg=-fuse-ld=lld";
            };
          };
        }
      );
    };
}
