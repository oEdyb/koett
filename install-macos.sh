#!/bin/zsh
set -euo pipefail

project_dir=${0:A:h}
app_path="/Applications/Koett.app"
legacy_app_path="/Applications/Local Voice Input.app"
stage_dir=$(mktemp -d)
stage_app="$stage_dir/Koett.app"
incoming_app="/Applications/.Koett-install-$$.app"
backup_app="/Applications/.Koett-backup-$$.app"
signing_identity=${KOETT_CODESIGN_IDENTITY:-}
new_app_installed=0
restore_legacy_on_failure=0

cleanup() {
    local exit_status=$?
    trap - EXIT
    rm -rf "$stage_dir" "$incoming_app"

    if (( exit_status != 0 && new_app_installed == 1 )); then
        rm -rf "$app_path"
    fi

    if [[ -e "$backup_app" ]]; then
        if (( exit_status == 0 )); then
            rm -rf "$backup_app"
        else
            mv "$backup_app" "$app_path"
            "$app_path/Contents/MacOS/koett" --register-login 2>/dev/null || true
            open -n -g "$app_path" 2>/dev/null || true
            print -u2 "The previous Koett version was restored."
        fi
    fi
    if (( exit_status != 0 && restore_legacy_on_failure == 1 )); then
        local legacy_restored=1
        "$legacy_app_path/Contents/MacOS/hold-to-talk" --register-login \
            2>/dev/null || legacy_restored=0
        open -n -g "$legacy_app_path" 2>/dev/null || legacy_restored=0
        if (( legacy_restored == 1 )); then
            print -u2 "The previous Local Voice Input app was restarted."
        else
            print -u2 "The previous Local Voice Input app could not be fully restored."
        fi
    fi
    exit "$exit_status"
}
trap cleanup EXIT

cd "$project_dir"
if ! command -v swift >/dev/null 2>&1; then
    print -u2 "Swift is required. Run: xcode-select --install"
    exit 1
fi
swift build -c release --product koett

resource_bundle=".build/release/Koett_Koett.bundle"
if [[ ! -d "$resource_bundle" ]]; then
    echo "Koett renderer resources were not built." >&2
    exit 1
fi

mkdir -p "$stage_app/Contents/MacOS" "$stage_app/Contents/Resources"
cp ".build/release/koett" "$stage_app/Contents/MacOS/koett"
cp "Packaging/Info.plist" "$stage_app/Contents/Info.plist"
ditto "$resource_bundle" \
    "$stage_app/Contents/Resources/Koett_Koett.bundle"

if [[ -z "$signing_identity" ]]; then
    signing_identity=$(security find-identity -v -p codesigning | awk \
        '/"Developer ID Application:/ && !found { print $2; found = 1 }')
fi
if [[ -z "$signing_identity" ]]; then
    signing_identity=$(security find-identity -v -p codesigning | awk \
        '/"Apple Development:/ && !found { print $2; found = 1 }')
fi

if [[ -z "$signing_identity" ]]; then
    signing_identity="-"
    print -u2 "Warning: no Apple Development signing identity was found."
    print -u2 "This ad-hoc build can require new macOS privacy approval after an update."
else
    echo "Signing Koett with a stable identity."
fi

codesign --force --options runtime \
    --entitlements "$project_dir/Packaging/Koett.entitlements" \
    --sign "$signing_identity" "$stage_app"
codesign --verify --deep --strict "$stage_app"

ditto "$stage_app" "$incoming_app"
codesign --verify --deep --strict "$incoming_app"

if [[ ! -e "$app_path" \
    && -x "$legacy_app_path/Contents/MacOS/hold-to-talk" ]]; then
    "$legacy_app_path/Contents/MacOS/hold-to-talk" --unregister-login
    restore_legacy_on_failure=1
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
    mv "$app_path" "$backup_app"
fi
mv "$incoming_app" "$app_path"
new_app_installed=1
codesign --verify --deep --strict "$app_path"

if [[ -f "$project_dir/.env" ]]; then
    groq_api_key=""
    while IFS= read -r line; do
        if [[ "$line" == GROQ_API_KEY=* ]]; then
            groq_api_key=${line#GROQ_API_KEY=}
            break
        fi
    done < "$project_dir/.env"
    if [[ -n "$groq_api_key" ]]; then
        GROQ_API_KEY="$groq_api_key" \
            "$app_path/Contents/MacOS/koett" --import-groq-key-if-missing
    fi
fi

"$app_path/Contents/MacOS/koett" --register-login
open -n -g "$app_path"

echo "Installed and started Koett."
