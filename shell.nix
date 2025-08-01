{
  pkgs ? import <nixpkgs> { },
}:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    pkg-config
    openssl.dev
    glib.dev
    atk.dev
    gtk3.dev
    libxkbcommon.dev

    libGL
    vulkan-headers
    vulkan-loader
    vulkan-tools
    vulkan-tools-lunarg
    vulkan-extension-layer
    vulkan-validation-layers

  ];
}
