#!/bin/zsh
set -euo pipefail

repo="${0:A:h:h}"
cache="${KOETT_MEETING_BUILD_CACHE:-$HOME/Library/Caches/koett-meeting-build}"
revision=96ec4d9823e9d0211944172da35f9f29134af990
hash=4d46e659151f31535b3947e322a58b0c604d3cbe93dfb284294e989a19d25ad7
mkdir -p "$cache"
archive="$cache/whisper-$revision.tar.gz"
if [[ ! -f "$archive" ]]; then
    curl -fsSL --retry 3 "https://codeload.github.com/ggml-org/whisper.cpp/tar.gz/$revision" -o "$archive.download"
    printf '%s  %s\n' "$hash" "$archive.download" | shasum -a 256 -c - >&2
    mv "$archive.download" "$archive"
fi
printf '%s  %s\n' "$hash" "$archive" | shasum -a 256 -c - >&2
tar -xzf "$archive" -C "$cache"
source_dir="$cache/whisper.cpp-$revision"
build="$cache/build-$revision"
cmake -S "$repo/Sources/MeetingASRWorker/Native" -B "$build" \
    -DWHISPER_SOURCE="$source_dir" -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_OSX_DEPLOYMENT_TARGET=15.0 -DCMAKE_OSX_ARCHITECTURES=arm64 >&2
cmake --build "$build" --parallel 4 --target KoettMeetingWhisper >&2
printf '%s\n' "$build/libKoettMeetingWhisper.dylib"
