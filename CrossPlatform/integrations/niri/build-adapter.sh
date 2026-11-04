#!/usr/bin/env bash
set -euo pipefail
source_dir=$(realpath "${1:?Pass a clean niri v26.04 source checkout}")
patch_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
case "$source_dir/" in /tmp/*) echo 'Build source must be outside /tmp' >&2; exit 1;; esac
cd -- "$source_dir"
[ "$(git rev-parse HEAD)" = 8ed0da44d974c32c6877d2f4630c314da0717ecb ] || { echo 'Expected niri v26.04 source commit' >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo 'Expected a clean source checkout' >&2; exit 1; }
git apply --check "$patch_dir/koett-pick-geometry-v1.patch"
git apply "$patch_dir/koett-pick-geometry-v1.patch"
export CARGO_BUILD_JOBS=4
export CARGO_TARGET_DIR="$source_dir/target"
exec systemd-run --user --scope -p MemoryMax=12G nice -n 10 \
    cargo build --locked --no-default-features
