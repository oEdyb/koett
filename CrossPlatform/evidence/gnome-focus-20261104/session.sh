#!/bin/sh
set -eu
export XDG_RUNTIME_DIR="$AGENT_LAB_RUNTIME/gnome"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
export XDG_CURRENT_DESKTOP=GNOME
export XDG_SESSION_DESKTOP=gnome
unset NIRI_SOCKET AT_SPI_BUS_ADDRESS
export GSETTINGS_BACKEND=keyfile
export GSETTINGS_SCHEMA_DIR="${KOETT_GNOME_LAB_ROOT:?Set the private GNOME lab root}/schemas"
export GNOME_SHELL_SESSION_MODE=user
python3 -c 'import os,json;from pathlib import Path;Path(os.environ["KOETT_GNOME_LAB_ROOT"]+"/runtime/session-env.json").write_text(json.dumps(dict(os.environ)))'
gsettings set org.gnome.shell enabled-extensions "['koett@olledyberg.com']"
gsettings set org.gnome.shell welcome-dialog-last-shown-version '51.1'
exec gnome-shell --headless --wayland --no-x11 --virtual-monitor=1280x800 --wayland-display=wayland-gnome-sol
