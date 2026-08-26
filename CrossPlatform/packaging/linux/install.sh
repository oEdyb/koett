#!/bin/sh
set -eu

PACKAGE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
BIN_DIR="$HOME/.local/bin"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"

install -d "$BIN_DIR" "$DATA_DIR/applications" "$DATA_DIR/icons/hicolor/scalable/apps"
install -m 755 "$PACKAGE_DIR/koett" "$BIN_DIR/koett"
install -m 644 "$PACKAGE_DIR/com.olledyberg.Koett.svg" \
  "$DATA_DIR/icons/hicolor/scalable/apps/com.olledyberg.Koett.svg"
sed "s|^Exec=.*|Exec=$BIN_DIR/koett|" \
  "$PACKAGE_DIR/com.olledyberg.Koett.desktop" \
  > "$DATA_DIR/applications/com.olledyberg.Koett.desktop"
chmod 644 "$DATA_DIR/applications/com.olledyberg.Koett.desktop"

echo "Koett is installed. Open Koett from your app menu."
