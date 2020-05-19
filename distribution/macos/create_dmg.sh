#!/bin/sh
test -f Emulsion-OSX.dmg && rm Emulsion-OSX.dmg
create-dmg \
  --volname "Emulsion Installer" \
  --volicon "resource_dev/emulsion.icns" \
  --background "distribution/macos/background.png" \
  --window-pos 200 120 \
  --window-size 750 600 \
  --icon-size 100 \
  --icon "Emulsion.app" 200 260 \
  --hide-extension "Emulsion.app" \
  --app-drop-link 560 260 \
  "Emulsion-Installer.dmg" \
  "target/release/bundle/osx/"
