{ stdenv
, lib
, fetchFromGitHub
, rustPlatform

, cmake
, gzip
, installShellFiles
, makeWrapper
, ncurses
, pkgconfig
, python3

, expat
, fontconfig
, freetype
, libGL
, libX11
, libXcursor
, libXi
, libXrandr
, libXxf86vm
, libxcb
, libxkbcommon
, wayland
, xdg_utils

  # Darwin Frameworks
, AppKit
, CoreGraphics
, CoreServices
, CoreText
, Foundation
, OpenGL
}:
let
  rpathLibs = [
    expat
    fontconfig
    freetype
    libGL
    libX11
    libXcursor
    libXi
    libXrandr
    libXxf86vm
    libxcb
  ] ++ lib.optionals stdenv.isLinux [
    libxkbcommon
    wayland
  ];
in
rustPlatform.buildRustPackage rec {
  pname = "emulsion";
  version = "0.0.0";
  #version = "7.2.0";
  #verArray = builtins.splitVersion version;

  src = fetchFromGitHub {
    owner = "ArturKovacs";
    repo = pname;
    rev = "897abfa08cf715b37fd157eb30bab2f4e0c0a55a";
    sha256 = "17dcl2g2p12z8byh86iiph9by1w0h57ww2n97x2ixghw02scsssr";
  };

  cargoSha256 = "0jmh8gmvf1diq5vs1pnr8v27v77q00x2bi526j7rn5z61km2f1h9";

  nativeBuildInputs = [
    cmake
    gzip
    installShellFiles
    makeWrapper
    ncurses
    pkgconfig
    python3
  ];

  buildInputs = rpathLibs
  ++ lib.optionals stdenv.isDarwin [
    AppKit
    CoreGraphics
    CoreServices
    CoreText
    Foundation
    OpenGL
  ];
  
  outputs = [ "out" ];
  
  installPhase = ''
    runHook preInstall
    install -D $releaseDir/emulsion $out/bin/emulsion
  '' + (
    if !stdenv.isDarwin then ''
      patchelf --set-rpath "${lib.makeLibraryPath rpathLibs}" $out/bin/emulsion
    '' else ''
    ''
  ) + ''
    runHook postInstall
  '';
  
  dontPatchELF = true;

  meta = with lib; {
    description = "A fast and minimalistic image viewer";
    homepage = "https://arturkovacs.github.io/emulsion-website/";
    license = licenses.mit;
    platforms = platforms.unix;
  };
}
