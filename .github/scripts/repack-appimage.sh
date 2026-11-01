#!/usr/bin/env bash
set -euo pipefail

appdir=target/release/bundle/appimage/Merframe.AppDir
appimage=$(realpath target/release/bundle/appimage/Merframe_*.AppImage)
tool=target/.tauri/appimagetool
runtime=target/.tauri/runtime-x86_64

# Prepared from the asset IDs and hashes in .github/build-inputs.json.
echo "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0  $tool" | sha256sum --check --strict
echo "617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46  $runtime" | sha256sum --check --strict

rm -f "$appdir"/usr/lib/libwayland-*.so* "$appdir"/usr/lib/libxkbcommon.so*
rm -rf "$appdir"/usr/lib/i386-linux-gnu
chmod +x "$tool"
rm -f "$appimage" "$appimage.sig"
ARCH=x86_64 "$tool" --appimage-extract-and-run --runtime-file "$runtime" "$appdir" "$appimage"
pnpm --dir app tauri signer sign "$appimage"
