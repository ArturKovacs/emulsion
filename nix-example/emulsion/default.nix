{ stdenv
, lib
, fetchFromGitHub
, rustPlatform

, installShellFiles
, makeWrapper
, pkgconfig
, python3

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
, Foundation
, OpenGL
}:
let
  rpathLibs = [
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

  cargoSha256 = "0jmh8gmvf1diq5vs1pnr8v27v77q00x2bi526j7rn5z61km2f1h9";

  nativeBuildInputs = [
    installShellFiles
    makeWrapper
    pkgconfig
    python3
  ];

  buildInputs = rpathLibs
  ++ lib.optionals stdenv.isDarwin [
    AppKit
    CoreGraphics
    CoreServices
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
