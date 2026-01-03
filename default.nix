{
  pkgs ? import <nixpkgs> { },
}:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "wanipaper";
  version = "1.0.0";

  src = pkgs.lib.cleanSource ./.;
  cargoLock.lockFile = "${src}/Cargo.lock";

  nativeBuildInputs = with pkgs; [
    pkg-config
    libxkbcommon.dev
  ];

  buildInputs = with pkgs; [
    pkg-config
    libxkbcommon.dev
  ];
}
