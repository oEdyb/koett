#!/bin/zsh
set -eu
repo_dir="${0:A:h:h}"
bin_dir="$HOME/.local/bin"
cli_link="$bin_dir/koett-transcribe"
if [[ -e "$cli_link" || -L "$cli_link" ]]; then
    if [[ ! -L "$cli_link" || "$(readlink "$cli_link")" != "$repo_dir/scripts/koett-transcribe" ]]; then
        print -u2 "Refusing to replace existing $cli_link"
        exit 1
    fi
fi
for dependency in python3 yt-dlp ffmpeg ffprobe; do
    command -v "$dependency" >/dev/null || { print -u2 "Missing $dependency"; exit 1; }
done
swift build --package-path "$repo_dir" -c release --product parakeet-baseline
mkdir -p "$bin_dir"
ln -sf "$repo_dir/scripts/koett-transcribe" "$cli_link"
print "Installed $cli_link"
print 'Usage: koett-transcribe "VIDEO_URL"'
