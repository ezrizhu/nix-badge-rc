{
  description = "EMLSS dev env";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs-esp-dev.url = "github:mirrexagon/nixpkgs-esp-dev";
  };
  outputs = { self, nixpkgs, rust-overlay, flake-utils, nixpkgs-esp-dev, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [
          rust-overlay.overlays.default
          nixpkgs-esp-dev.overlays.default
        ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
          extensions = [ "rust-src" "miri" "rustfmt" ];
          targets = ["riscv32imc-unknown-none-elf"];
        });
        espIdf = pkgs.esp-idf-full;
      in
      {
        devShells.default = pkgs.mkShell {
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          buildInputs = with pkgs; [
            openssl
            pkg-config
            fd
            rustToolchain
            (pkgs.callPackage ./espflash.nix {})
            ldproxy
            cargo-generate
            python3
            cmake
            ninja
            python311
            python3Packages.pip
            python3Packages.virtualenv
            espIdf
            probe-rs-tools
          ];
          shellHook = ''
          export ESP_IDF_TOOLS_INSTALL_DIR=fromenv
          export PATH="$IDF_PYTHON_ENV_PATH/bin:$PATH"
          '';
        };
      }
    );
}
