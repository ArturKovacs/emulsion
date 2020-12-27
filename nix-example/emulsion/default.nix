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
  version = "7.2.0";
  verArray = builtins.splitVersion version;

  src = fetchFromGitHub {
    owner = "ArturKovacs";
    repo = pname;
    rev = "v${builtins.elemAt verArray 0}.${builtins.elemAt verArray 1}";
    sha256 = "1king04p5j4gsvprrfppwaxa5jn4ga4nc0v63wl6fvq2ngwnkg4g";
  };

  cargoSha256 = "0dg331v2wq0dwb1drrdpipb6gvc4kah6mli04y8pbakgfb0v6rbd";

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
