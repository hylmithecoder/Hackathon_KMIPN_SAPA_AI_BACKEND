{ pkgs ? import <nixpkgs> {} }:

let
  musl = pkgs.pkgsCross.musl64;
  muslTarget = musl.stdenv.hostPlatform.config;
  muslCc = "${musl.stdenv.cc}/bin/${muslTarget}-cc";
  muslAr = "${musl.stdenv.cc}/bin/${muslTarget}-ar";
  cargoMusl = pkgs.writeShellScriptBin "cargo-musl" ''
    export CC_${builtins.replaceStrings ["-"] ["_"] muslTarget}="${muslCc}"
    export AR_${builtins.replaceStrings ["-"] ["_"] muslTarget}="${muslAr}"
    export CARGO_TARGET_${pkgs.lib.toUpper (builtins.replaceStrings ["-"] ["_"] muslTarget)}_LINKER="${muslCc}"
    exec "${musl.buildPackages.cargo}/bin/cargo" "$@"
  '';
in
pkgs.mkShell {
  nativeBuildInputs = (with pkgs; [
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer
    gcc
    gnumake
    pkg-config
    cmake
    openssl
  ]) ++ [
    cargoMusl
  ];

  shellHook = ''
    # Unset LD_LIBRARY_PATH if present to prevent GLIBC mismatches between host OS and Nix packages
    unset LD_LIBRARY_PATH

    echo ""
    echo "  Rust + Axum development shell"
    echo "  ─────────────────────────────"
    echo "  gunakan \`cargo build\` / \`cargo run\` seperti biasa"
    echo "  gunakan \`make build-static\` untuk binary musl statis"
    echo ""
  '';
}
