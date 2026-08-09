#!/bin/zsh
set -euo pipefail

project_dir=${0:A:h}
app_path="/Applications/Koett.app"
legacy_app_path="/Applications/Local Voice Input.app"
stage_dir=$(mktemp -d)
stage_app="$stage_dir/Koett.app"
signing_identity=${KOETT_CODESIGN_IDENTITY:-}

trap 'rm -rf "$stage_dir"' EXIT

cd "$project_dir"
swift build -c release --product koett

mkdir -p "$stage_app/Contents/MacOS"
cp ".build/release/koett" "$stage_app/Contents/MacOS/koett"
cp "Packaging/Info.plist" "$stage_app/Contents/Info.plist"

if [[ -z "$signing_identity" ]]; then
    signing_identity=$(security find-identity -v -p codesigning | sed -nE \
        's/^[[:space:]]*[0-9]+\) ([0-9A-F]+) "Apple Development:.*$/\1/p' | head -n 1)
fi

if [[ -z "$signing_identity" ]]; then
    signing_identity="-"
    print -u2 "Warning: no Apple Development signing identity was found."
    print -u2 "This ad-hoc build can require new macOS privacy approval after an update."
else
    echo "Signing Koett with a stable Apple Development identity."
fi

codesign --force --sign "$signing_identity" "$stage_app"

if [[ -x "$legacy_app_path/Contents/MacOS/hold-to-talk" ]]; then
    "$legacy_app_path/Contents/MacOS/hold-to-talk" --unregister-login
    osascript -e 'tell application id "com.olledyberg.LocalVoiceInput" to quit' 2>/dev/null || true
    for _ in {1..20}; do
        if ! pgrep -f '^/Applications/Local Voice Input\.app/Contents/MacOS/hold-to-talk$' >/dev/null; then
            break
        fi
        sleep 0.25
    done
    if pgrep -f '^/Applications/Local Voice Input\.app/Contents/MacOS/hold-to-talk$' >/dev/null; then
        echo "Local Voice Input did not stop. Koett was not installed." >&2
        exit 1
    fi
fi

if [[ -e "$app_path" ]]; then
    "$app_path/Contents/MacOS/koett" --unregister-login
    osascript -e 'tell application id "com.olledyberg.Koett" to quit' 2>/dev/null || true
    for _ in {1..20}; do
        if ! pgrep -f '^/Applications/Koett\.app/Contents/MacOS/koett$' >/dev/null; then
            break
        fi
        sleep 0.25
    done
    if pgrep -f '^/Applications/Koett\.app/Contents/MacOS/koett$' >/dev/null; then
        echo "Koett did not stop. The update was cancelled." >&2
        exit 1
    fi
    ditto "$stage_app" "$app_path"
else
    mv "$stage_app" "$app_path"
fi

"$app_path/Contents/MacOS/koett" --register-login
open -n -g "$app_path"

echo "Installed and started Koett."
