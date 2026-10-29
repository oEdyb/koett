# Media operation evidence

Implemented on `feat/linux-ui-history`, uncommitted. Evidence directory label is
20261030; the machine clock during execution was 2026-10-29. Existing uncommitted
model/formatting work was preserved. No install, preferences on the host, commit,
push, subagent, protected URLs, cookies or personal transcripts were used.

## Reproduce

- `koett media 'https://example.com/public-media'` dispatches to the running
  desktop engine; it does not start another recognizer. `koett cancel-media`
  cancels it. The running application must be in the same desktop session.
- GTK Menu → Transcribe media, or Ctrl-Shift-T inside the app, opens URL entry.
  Enter submits; Cancel/Escape/window close cancels. The result window and
  existing recovery pill provide exact Copy. Media does not paste or format.
- `scripts/test_linux_media.py --lab koett-media-operation --binary
  target/release/koett --output NEW_DIRECTORY --theme dark` runs only inside
  that private lab. Restart the lab between runs to refresh its accessibility
  bus. It uses the public model sample in fixtures/tmp/media-operation/public.wav.
- `examples/media_probe.rs MODEL_DIRECTORY URL` exercises runtime cancel,
  warmed-worker reuse, real fetch/FFmpeg/ASR, Unicode metadata, exact text and
  forced primary-storage fallback in an isolated XDG data directory. Each run
  needs a fresh XDG directory: the fallback fixture intentionally blocks the
  primary media destination. `--reject URL` exercises a rejected fetch.

## Selected evidence

- `bounded-dark/` and `bounded-light/`: actual native URL, fetching, cancelled,
  Copy focus, copied results, 320-pixel narrow results, keyboard focus, error and
  post-cancellation capture PNGs. Their checks.json compares clipboard bytes to
  the separately saved media result. Captures inspected; no screenshot parity
  or appearance approval claimed. The Copy button is in the existing recovery
  surface as well as the native result window; integrated Mac pill parity is
  still pending.
- `probe-bounded.log`: real loopback public WAV → installed yt-dlp → FFmpeg →
  already-warm selected 110M worker; cancellation then successful request;
  exact text, Unicode metadata and shared failed-transcript storage verified.
- `redirect.log`: a controlled HTTP 302 to the owned fixture's file URL was
  requested and rejected by the actual operation. No personal file targeted.
- `tests.log`: 108 passed, three existing ignored. New tests cover URL boundary,
  owned child success/nonzero exit/timeout/cancel, temporary cleanup.
- `fmt.log`, `clippy.log`, `release.log`, `msrv.log`, `windows.log`, `ruff.log`,
  `pycompile.log`, `diff-check.log`: successful required checks. Rust uses
  SHERPA_ONNX_LIB_DIR=/tmp/koett-native-host-abi0/lib. Windows uses DOCS_RS=1,
  LIBSQLITE3_SYS_USE_PKG_CONFIG=1, PKG_CONFIG_ALLOW_CROSS=1. MSRV is 1.92.0;
  Ruff is 0.17.2. Windows is a source check, not runtime proof.

## Bounds and compatibility

Installed tools: ~/.local/bin/yt-dlp and /usr/bin/ffmpeg. The operation ignores
configuration/cookies, selects one non-live non-playlist item, restricts selected
formats to direct HTTP(S), and disables FFmpeg network protocols and manifest
formats. FFmpeg produces mono 16 kHz signed 16-bit PCM. Input is checked against
2 GiB; output against three hours. Per-child RLIMIT_FSIZE enforces the exact source/PCM file bounds before writes
can exceed them; a deterministic synthetic child test verifies this. A 50 ms
directory-size watchdog additionally bounds aggregate temporary outputs. Socket timeout is 30 seconds; fetch plus conversion
share a 30-minute deadline. ASR reads fixed 30-second chunks and checks cancellation
between decode calls. A cancelled decode finishes its current bounded call;
no model is killed/reloaded. Audio temporary directories use RAII cleanup.

Manifest-only HLS/DASH, DRM, login/cookie-dependent sources and sites blocking
anonymous requests are explicit compatibility limits. Full three-hour accuracy,
2 GiB network stress, arbitrary-site compatibility, focused-browser detection,
configurable global media shortcut, Open Media Transcripts folder action and
Mac compact result geometry remain unverified/pending as the inventory states.
No keys, profile/debug-port reading or clipboard borrowing was introduced.

Historical failed harness runs remain for diagnosis. Early AT-SPI actions failed
on an unassociated window and a disabled-state race; both were fixed. Keyboard
input/Copy then passed in selected captures. A reused probe fixture intentionally
left its primary path blocked; probe-final.log failed its primary-file assertion.
The fresh isolated run in probe-bounded.log passed without weakening assertions.
Synthetic cancelled HTTP handlers can log BrokenPipe when their peer is killed;
the subsequent successful request and exact Copy are checked separately.
The private lab and its owned subprocesses were stopped after verification.

Primary references checked during implementation:
https://github.com/yt-dlp/yt-dlp#usage-and-options
https://ffmpeg.org/ffmpeg-protocols.html
https://ffmpeg.org/ffmpeg-formats.html
Local canonical behavior: docs/feature-contracts.md, actual Sources/Koett media
files and docs/mac-parity-reference-2026-10-29.md.
