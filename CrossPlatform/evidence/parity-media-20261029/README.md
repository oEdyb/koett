# Formatting post-review prerequisite proof, 2026-10-29

This is **not a completed media increment**. No application source was changed by
this run. Existing uncommitted model/formatting work was preserved. No install,
commit, push, cloud request, private media, host GUI/input/clipboard/audio test,
secret access, or subagent was used.

## Proof

Private `agent-lab koett-media`, rebuilt `target/debug/koett`, isolated XDG state,
110M official public test WAV and the existing local S1-mini runtime/model.

1. Select S1-mini, SIGSTOP the app-owned llama-server during setup, open Settings
   while preferences still have formatting disabled, then SIGCONT. Wait for enabled
   preferences, Save the stale Settings form, assert enabled remains and the same
   app-owned server PID remains alive.
2. Make only the lab preferences directory read-only. Select Raw. Assert visible
   S1-mini save error, enabled preferences remain and Output becomes sensitive.
   Inspect `readonly-selection-open.png` and `readonly-restored-s1.png`: the
   displayed value restores S1-mini. Accessibility does not expose the combo's
   selected text in its descendant names, so selection itself is visually verified.
3. While still read-only, dictate the official model fixture through private audio.
   Assert exactly one new SQLite record with nonempty final and raw text. Exact
   observed text is in `proof.json`; it confirms cleanup actually ran.
4. Restore writable preferences, select Raw, assert disabled is persisted and the
   app-owned llama-server exits. Inspect `writable-raw.png`.

These screenshots establish those specific control states only, not visual parity
or appearance approval. No existing test/assertion was edited or weakened.

Reproduce (choose a new nonexistent output directory):

```sh
agent-lab start koett-media
agent-lab run koett-media -- env \
  PYTHONPATH=/home/olle/dev/koett/CrossPlatform/scripts \
  PATH=/tmp/koett-format-eval/llama-b11981:/home/olle/.local/bin:/usr/bin \
  LD_LIBRARY_PATH=/tmp/koett-native-host-abi0/lib \
  /usr/bin/python3 /home/olle/dev/koett/CrossPlatform/evidence/parity-media-20261029/formatting-postreview-fixture.py \
  --lab koett-media --binary /home/olle/dev/koett/CrossPlatform/target/debug/koett \
  --output /tmp/koett-media-format-proof-new --theme light
agent-lab stop koett-media
```

The saved harness expands the existing model fixture helpers; it does not execute
source fragments dynamically. Raw isolated state remains under `/tmp`.

## Current-source gates

- `cargo fmt --check`: pass.
- `cargo test --locked`: 104 passed, 3 ignored; proxy subprocess regression also passed.
- `cargo clippy --locked --all-targets -- -D warnings`: pass.
- `cargo build --release --locked`: pass.
- `cargo +1.92.0 check --locked`: pass, separate `/tmp/koett-msrv-1.92` target.
- Windows `cargo check --locked --target x86_64-pc-windows-msvc`: pass with
  `DOCS_RS=1 LIBSQLITE3_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1`.
- Host gates use `SHERPA_ONNX_LIB_DIR=/tmp/koett-native-host-abi0/lib`.
- Ruff exactly 0.17.2 and py_compile on saved harness: pass.

Logs are alongside this receipt. These check the existing model/formatting source,
not a media implementation. Independent reviews remain parent-owned.

## Media investigation and unfinished scope

Installed tools: yt-dlp **2026.03.13**; FFmpeg **n9.0.2**. Read actual local
`BrowserMedia.swift`, `MediaSession.swift`, `MediaTranscriptStore.swift`, and
`RecordingOverlay.swift`, feature contracts and parity docs. The local media
store source uses SQLite, while the pinned Mac reference describes Markdown;
the explicit task requires separate media files and unchanged history schema.

Primary sources consulted:

- https://github.com/yt-dlp/yt-dlp (configuration and CLI options)
- https://ffmpeg.org/ffmpeg-protocols.html (protocol whitelist)
- https://gnome.pages.gitlab.gnome.org/at-spi2-core/libatspi/iface.Document.html
- https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html

AT-SPI exposes document attributes but this inspection does not establish a
portable, reliable focused-tab URL contract. No real browser capability test was
completed; automatic focused-tab support is **unverified**, not proven impossible.
No debug port, extension, cookie/profile access or clipboard URL borrowing occurred.

Still unimplemented/unverified: manual URL UI/menu/CLI and structured status,
redirect/DNS/SSRF and protocol restrictions at every downloader connection,
one-item/no-cookie/no-live/3h/2GiB/time limits, owned process-group cancellation
and reaping, bounded output, conversion, warm ASR/session admission, temporary
cleanup, media metadata/files/shared durable recovery/exact Copy, shortcuts, and
the full light/dark/narrow/keyboard/busy/error/cancel/result screenshot matrix.
Do not expose an unrestricted downloader as a completed boundary. Ask/TTS,
experimental engines, complete menus/recovery and packaging remain pending as
listed in the parity inventory. No whole-port or appearance approval is claimed.
