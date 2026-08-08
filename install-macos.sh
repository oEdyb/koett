#!/bin/zsh
set -euo pipefail

project_dir=${0:A:h}
app_path="/Applications/Local Voice Input.app"
stage_dir=$(mktemp -d)
stage_app="$stage_dir/Local Voice Input.app"

trap 'rm -rf "$stage_dir"' EXIT

cd "$project_dir"
swift build -c release --product hold-to-talk

mkdir -p "$stage_app/Contents/MacOS"
cp ".build/release/hold-to-talk" "$stage_app/Contents/MacOS/hold-to-talk"
cp "Packaging/Info.plist" "$stage_app/Contents/Info.plist"
codesign --force --sign - "$stage_app"

if [[ -e "$app_path" ]]; then
    "$stage_app/Contents/MacOS/hold-to-talk" --unregister-login
    osascript -e 'tell application id "com.olledyberg.LocalVoiceInput" to quit' 2>/dev/null || true
    sleep 1
    ditto "$stage_app" "$app_path"
else
    mv "$stage_app" "$app_path"
fi

"$app_path/Contents/MacOS/hold-to-talk" --register-login
open -n -gj "$app_path"

echo "Installed and started Local Voice Input."
