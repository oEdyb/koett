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
embedded="$app_path/Contents/Frameworks/Sparkle.framework"
sign_options=(--force --options runtime --sign "$identity")
[[ "$identity" == - ]] || sign_options+=(--timestamp)
codesign "${sign_options[@]}" "$embedded/Versions/B/XPCServices/Installer.xpc"
codesign "${sign_options[@]}" --preserve-metadata=entitlements "$embedded/Versions/B/XPCServices/Downloader.xpc"
codesign "${sign_options[@]}" "$embedded/Versions/B/Autoupdate"
codesign "${sign_options[@]}" "$embedded/Versions/B/Updater.app"
codesign "${sign_options[@]}" "$embedded"
codesign --verify --deep --strict "$embedded"
