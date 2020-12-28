with import <nixpkgs> {};

callPackage ./emulsion {
  inherit (xorg) libXcursor libXxf86vm libXi;
  inherit (darwin.apple_sdk.frameworks) AppKit CoreGraphics CoreServices Foundation OpenGL;
}

# let
#   pkgs = nixpkgs.pkgs;
#   lib = nixpkgs.lib;
# in callPackage ./emulsion {
#   inherit (xorg) libXcursor libXxf86vm libXi;
#   inherit (darwin.apple_sdk.frameworks) AppKit CoreGraphics CoreServices CoreText Foundation OpenGL;
# }
