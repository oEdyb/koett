# Finite HLS/DASH backend verification

Scope: `src/media_transcription.rs`, `scripts/test_linux_media_manifests.py`, and this evidence directory. Base checkpoint: 925f0b4. Public Fetch/Download/save and history schema remain unchanged. No host tool replacement, dependency edits, installation, commits, or pushes.

## Mechanism

Select a single best audio or combined format using HTTP(S), m3u8_native, or http_dash_segments. yt-dlp downloads natively into a private 0700 operation directory; missing fragments abort instead of silently truncating speech. Output uses source.%(ext)s and after_move JSON carries exact title and final path. Accept only a regular, non-symlink source file directly inside that directory. FFmpeg accepts only local file protocols and listed media demuxers, now including MPEG-TS. No remote conversion or manifest demuxer is enabled.

The operation gives yt-dlp a nonexistent private --ffmpeg-location and --fixup never: upstream HlsFD otherwise delegates unsupported HLS features to remote FFmpeg even with --downloader native. This disables that fallback without changing host FFmpeg or configuration. The separate bounded /usr/bin/ffmpeg conversion remains available.

Existing product limits remain: 2 GiB per downloaded file, three-hour PCM cap, 30-minute fetch/conversion deadline, 30-second socket timeout, owned process groups, 50 ms aggregate directory watchdog, RAII cleanup, and cooperative cancellation. Fragment retries are bounded at two, matching request retries. No playlist/live/cookie/file-input support is added.

## Provenance

The product-selected ~/.local/bin/yt-dlp resolves to ~/.local/share/uv/tools/yt-dlp/bin/yt-dlp, version 2026.03.13, a Python console launcher. PATH happens to select the same version. No candidate or upgrade was required. binaries.sha256 hashes the actual launcher, FFmpeg, and frozen tested app. downloader-source-hashes.json hashes every installed yt_dlp Python source file, rather than treating the small launcher hash as a package hash. Upstream license: Unlicense; no upstream binary/source is redistributed here.

Primary references checked:
- https://github.com/yt-dlp/yt-dlp (current options: native downloader, fragment abort, output templates, fixup)
- https://raw.githubusercontent.com/yt-dlp/yt-dlp/2026.03.13/yt_dlp/downloader/hls.py
- https://raw.githubusercontent.com/yt-dlp/yt-dlp/2026.03.13/yt_dlp/downloader/dash.py
- Installed yt_dlp extractor/generic.py and postprocessor/ffmpeg.py (live detection and disabling fallback).
- docs/feature-contracts.md and Sources/Koett/BrowserMedia.swift.

Frozen app: ~/.cache/koett/private-tests/manifests-bin/koett, SHA-256 79b21900d61b64435255b1c15e64cb1f953f74fa8a42d52347c14b1885df007c. Copied before GUI runs; subsequent shared builds cannot change it. It includes this production backend patch; later backend changes only add unit tests. Parent must rebuild/freeze an integrated binary after browser/UI integration.

## Fixture and reproduction

All source servers bind 127.0.0.1. scripts/test_linux_media_manifests.py builds AAC HLS VOD and static DASH from the cached public model test_wavs/0.wav using /usr/bin/ffmpeg, two-second segments, 96 kbit/s AAC. fixtures.sha256 identifies the original public WAV, model and tokens. Required model files are copied to private fixtures; user model files are not modified.

The isolated desktop is agent-lab koett-media-manifests. Run the script with --lab koett-media-manifests --binary <frozen app> --output <fresh private cache directory> --theme light --manifest hls (or dark/dash), through agent-lab run. Set TMPDIR=~/.cache/koett/private-tests and scoped LD_LIBRARY_PATH=/tmp/koett-gtk-upstream-fix/runtime/lib. Remove NIRI_SOCKET, XDG_ACTIVATION_TOKEN and DESKTOP_STARTUP_ID. The harness owns private XDG directories, clipboard/input, DBus/AT-SPI and Pulse microphone monitor. No physical audio or cloud provider calls.

## Gates

Six focused backend tests passed: process exit/timeout/cancel; per-file bound; final filename/title boundary and multiple-result rejection; aggregate fragment bound and cleanup; drop cleanup; URL boundary. Strict all-target Clippy, locked release, Rust 1.92 source check, fmt and diff check passed. Windows source check passed with DOCS_RS=1 LIBSQLITE3_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1 (not a Windows executable/runtime test). The plain Windows check lacks lib.exe; windows.log retains that diagnostic. Focused Python Ruff uses the existing harness convention --ignore E402 for GI initialization imports; compileall passed. Parent owns the full suite.

Earlier *-final/*-verified logs include harness failures: incorrect expected error-label prefix, overlapping prior app shutdown, and issuing recording before retry ASR reached Ready. These were harness errors; final checks wait for Ready and execute sequentially.

Unsupported: live HLS/dynamic DASH and RSS playlists are rejected by actual product checks. File/credential URLs are rejected by URL boundary tests. DRM, protected/login-required sites, arbitrary websites, encrypted HLS variants, and live streams are not supported claims; no authentication, decryption or remote FFmpeg workaround was enabled. Full 2 GiB/three-hour network stress is not run; focused reduced-bound tests exercise the actual limiter functions without changing product defaults.

Final product outcomes: hls-checks.json and dash-checks.json passed every assertion. Both completed segment-request cancellation, real warm ASR with readable Phoebe text, exact Copy, exact source/title, direct HTTP, missing-segment/live/playlist rejection, retry, exact save recovery, and real private dictation after cancellation. hls-complete.log and dash-verified.log are the final successful runs. Copy screenshots were inspected. Final harness teardown verified no operation directory or owned yt-dlp/FFmpeg argument remained.

## Parent review

Two fresh-context reviews completed. Review B found no actionable issue. Review A found that final-file validation compared a canonical file against a noncanonical operation directory, rejecting a valid symlinked temporary root. Parent normalized both sides and extended the final-file regression to exercise an actual directory symlink. All six focused backend tests, formatting and diff checks passed after that fix. The existing GUI receipts predate this path-normalization fix; the final integrated browser/media build and full gates remain parent-owned.
