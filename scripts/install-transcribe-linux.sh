#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
bin_dir="$HOME/.local/bin"
cli_link="$bin_dir/koett-transcribe"
if [[ -e "$cli_link" || -L "$cli_link" ]]; then
    if [[ ! -L "$cli_link" || "$(readlink "$cli_link")" != "$repo_dir/scripts/koett-transcribe" ]]; then
        echo "Refusing to replace existing $cli_link" >&2
        exit 1
    fi
fi
for dependency in python3 yt-dlp ffmpeg ffprobe cargo; do
    command -v "$dependency" >/dev/null || { echo "Missing $dependency" >&2; exit 1; }
done
cd "$repo_dir/CrossPlatform"
systemd-run --user --scope -p MemoryMax=12G nice -n 10 env CARGO_BUILD_JOBS=4 cargo build --release --locked --bin koett-transcribe-engine
mkdir -p "$bin_dir"
ln -sfn "$repo_dir/scripts/koett-transcribe" "$cli_link"
echo "Installed $cli_link"
