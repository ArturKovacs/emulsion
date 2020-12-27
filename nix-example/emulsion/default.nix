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
    rev = "3d93d5e74554e0dfd92f2b1642ef952fbe34682e";
    sha256 = "08banc6qxdmjvkbl3cd6wyl110r3ji49ra0xx38y1xab28kakrga";
  };

  cargoSha256 = "1msd7v8vfx2v4jwp9xx90m78k3awli4w5gmwshdf1jpq44f023s4";

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
