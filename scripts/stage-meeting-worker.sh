#!/bin/zsh
set -euo pipefail
app="$1"
products="$2"
library="$3"
identity="$4"
repo="${0:A:h:h}"

mkdir -p "$app/Contents/Frameworks" "$app/Contents/Resources"
cp "$products/koett-meeting-asr" "$app/Contents/MacOS/koett-meeting-asr"
cp "$library" "$app/Contents/Frameworks/libKoettMeetingWhisper.dylib"
ditto "$products/Koett_MeetingASRProtocol.bundle" "$app/Contents/Resources/Koett_MeetingASRProtocol.bundle"
ditto "$repo/Sources/MeetingASRWorker/Native/Notices" "$app/Contents/Resources/Meeting ASR Notices"
sign_options=(--force --sign "$identity")
if [[ "$identity" != - ]]; then
    sign_options+=(--options runtime --timestamp)
fi
codesign "${sign_options[@]}" "$app/Contents/Frameworks/libKoettMeetingWhisper.dylib"
codesign "${sign_options[@]}" "$app/Contents/MacOS/koett-meeting-asr"
