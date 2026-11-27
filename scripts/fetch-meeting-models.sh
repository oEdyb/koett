#!/bin/zsh
set -euo pipefail

cache="${1:-$HOME/Library/Caches/koett-meeting-models}"
manifest="${0:A:h:h}/Sources/MeetingASRProtocol/Resources/Models.tsv"
while IFS=$'\t' read -r group repo revision relative size hash; do
    output="$cache/$relative"
    if [[ "$group" == v3 ]]; then
        output="$cache/parakeet-tdt-0.6b-v3/$relative"
    fi
    mkdir -p "${output:h}"
    if [[ -f "$output" ]] && [[ "$(stat -f %z "$output")" == "$size" ]] && \
        printf '%s  %s\n' "$hash" "$output" | shasum -a 256 -c -; then
        continue
    fi

    temporary="$(mktemp "${output}.download.XXXXXXXX")"
    trap 'rm -f "$temporary"' EXIT
    curl -fsSL --retry 3 "https://huggingface.co/$repo/resolve/$revision/$relative?download=true" -o "$temporary"
    [[ "$(stat -f %z "$temporary")" == "$size" ]]
    printf '%s  %s\n' "$hash" "$temporary" | shasum -a 256 -c -
    mv -f "$temporary" "$output"
    trap - EXIT
done < "$manifest"
