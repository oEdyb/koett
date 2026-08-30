#!/bin/zsh

set -euo pipefail

cache_root="${1:-${HOME}/Library/Caches/koett-benchmarks}"
fleurs_root="${cache_root}/fleurs-sv"
whisper_root="${cache_root}/whisper.cpp"

fleurs_revision="70bb2e84b976b7e960aa89f1c648e09c59f894dd"
fleurs_tsv_sha256="55f48c5385a6e5fb8a62ea90212c04b005e2f77d7bd8fcf20bc3a5bda223aae2"
fleurs_archive_sha256="3792fd432675e16d85a67f5caf9927ad608aefc2484d738f26704f75584a5a6f"
kb_revision="1499d2d2f0c7ed545bd6f2eec85287cf8d8c8b38"
kb_model_sha256="aead29b356bca8840e72a8dc2286e2d69e6702639751a1e60cb3c8eacefec546"
whisper_revision="371b5a7561823ab2bb32142d2751e35e7534727b"

download_verified() {
    local url="$1"
    local destination="$2"
    local expected_sha256="$3"

    if [[ -f "${destination}" ]] && \
        printf '%s  %s\n' "${expected_sha256}" "${destination}" | shasum -a 256 -c - >/dev/null; then
        return
    fi

    curl -fL --retry 5 --continue-at - --output "${destination}.download" "${url}"
    printf '%s  %s\n' "${expected_sha256}" "${destination}.download" | shasum -a 256 -c -
    mv -f "${destination}.download" "${destination}"
}

mkdir -p "${fleurs_root}"

download_verified \
    "https://huggingface.co/datasets/google/fleurs/resolve/${fleurs_revision}/data/sv_se/test.tsv" \
    "${fleurs_root}/test.tsv" \
    "${fleurs_tsv_sha256}"
download_verified \
    "https://huggingface.co/datasets/google/fleurs/resolve/${fleurs_revision}/data/sv_se/audio/test.tar.gz" \
    "${fleurs_root}/test.tar.gz" \
    "${fleurs_archive_sha256}"
download_verified \
    "https://huggingface.co/KBLab/kb-whisper-base/resolve/${kb_revision}/ggml-model-q5_0.bin" \
    "${cache_root}/kb-whisper-base-q5_0.bin" \
    "${kb_model_sha256}"

if [[ ! -d "${fleurs_root}/audio/test" ]]; then
    mkdir -p "${fleurs_root}/audio"
    tar -xzf "${fleurs_root}/test.tar.gz" -C "${fleurs_root}/audio"
fi

if [[ ! -d "${whisper_root}/.git" ]]; then
    git clone --no-checkout https://github.com/ggml-org/whisper.cpp.git "${whisper_root}"
fi
git -C "${whisper_root}" fetch --depth 1 origin "${whisper_revision}"
git -C "${whisper_root}" checkout --detach "${whisper_revision}"

cmake \
    -S "${whisper_root}" \
    -B "${whisper_root}/build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DWHISPER_BUILD_TESTS=OFF \
    -DWHISPER_BUILD_SERVER=OFF \
    -DWHISPER_BUILD_EXAMPLES=ON
cmake --build "${whisper_root}/build" --config Release -j 4 --target whisper-cli

printf 'FLEURS TSV: %s\n' "${fleurs_root}/test.tsv"
printf 'FLEURS audio: %s\n' "${fleurs_root}/audio/test"
printf 'KB-Whisper model: %s\n' "${cache_root}/kb-whisper-base-q5_0.bin"
printf 'whisper.cpp CLI: %s\n' "${whisper_root}/build/bin/whisper-cli"
