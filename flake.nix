{
  description = "larkline — the line to all your tools";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Read version from Cargo.toml so bumping the package doesn't require
        # editing the flake.
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        larkline = pkgs.rustPlatform.buildRustPackage {
          pname = "larkline";
          version = cargoToml.package.version;

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          # mlua with `vendored` + `lua54` compiles bundled Lua sources; needs
          # a C toolchain (provided by stdenv) and pkg-config for discovery.
          nativeBuildInputs = [ pkgs.pkg-config ];

          # No external C deps — mlua vendors Lua, reqwest uses rustls-tls.
          buildInputs = [ ];

          meta = with pkgs.lib; {
            description = cargoToml.package.description;
            homepage = cargoToml.package.homepage;
            license = licenses.mit;
            maintainers = [ ];
            mainProgram = "lark";
            platforms = platforms.unix;
          };
        };
      in
      {
        packages.default = larkline;
        packages.larkline = larkline;

        apps.default = flake-utils.lib.mkApp {
          drv = larkline;
          name = "lark";
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.rustc
            pkgs.cargo
            pkgs.rust-analyzer
            pkgs.clippy
            pkgs.rustfmt
            pkgs.pkg-config
          ];
        };
      });
}
