#!/bin/zsh
set -euo pipefail
project_dir=${0:A:h:h}
app_path=${1:?Pass a staged .app path}
identity=${2:?Pass a signing identity}
framework="$project_dir/.build/artifacts/sparkle/Sparkle/Sparkle.xcframework/macos-arm64_x86_64/Sparkle.framework"
[[ -d "$framework" ]] || { print -u2 "Build Koett first to fetch pinned Sparkle."; exit 1; }
mkdir -p "$app_path/Contents/Frameworks" "$app_path/Contents/Resources"
ditto "$framework" "$app_path/Contents/Frameworks/Sparkle.framework"
cp "$project_dir/.build/artifacts/sparkle/Sparkle/LICENSE" "$app_path/Contents/Resources/Sparkle-LICENSE.txt"
codesign --force --options runtime --preserve-metadata=identifier,entitlements \
    --sign "$identity" "$app_path/Contents/Frameworks/Sparkle.framework"
codesign --verify --deep --strict "$app_path/Contents/Frameworks/Sparkle.framework"
