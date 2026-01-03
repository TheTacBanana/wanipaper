{
  pkgs ? import <nixpkgs> { },
}:

pkgs.rustPlatform.buildRustPackage {
  pname = "wanipaper";
  version = "1.0.0";

  src = pkgs.lib.cleanSource ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = with pkgs; [
    pkg-config
  ];

  buildInputs = with pkgs; [
    libxkbcommon
  ];
}
