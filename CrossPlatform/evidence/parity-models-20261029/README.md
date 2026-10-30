# First model/control increment — 2026-10-29

Shared checkout: `feat/linux-ui-history`. No commit, push, installation or live
preference change. Full generated fixtures remain under `/tmp/koett-parity-*`.
The owned `agent-lab koett-parity` desktop is stopped.

## Fresh gates

From `CrossPlatform/`, use the existing ASR-only native library receipt:
`SHERPA_ONNX_LIB_DIR=/tmp/koett-native-host-abi0/lib`.

- `cargo fmt --check`: pass.
- `cargo test --locked`: **96 passed, 3 ignored**, no downloads or large-model choice.
- `cargo clippy --locked --all-targets -- -D warnings`: pass.
- `cargo build --release --locked`: pass.
- `DOCS_RS=1 LIBSQLITE3_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1 cargo check --locked --target x86_64-pc-windows-msvc`: pass, source check only.
- Ruff 0.17.2 and py_compile for the four affected native UI harnesses: pass.
- `git diff --check`: pass.

Native library/vendor receipts, Cargo dependencies, Swift/Windows behavior and
SQLite schema are preserved. Windows receives the exhaustive active-model update
match. No Mac compilation or native Windows runtime claim is made.

## Native proof

`model-journey/checks.json` and `inference.json` record real verified v2 setup,
cancellation retaining compact selection, invalid archive/cache recovery retaining
the working compact worker, disabled choices during private recording, saved v2
restart and Settings save preserving the selected model. Both public-fixture
inferences ran with the built engine; 110M reports Phoebe and v2 reports Phebe,
with separate model directories and load timing. These are actual independent
recognizers, not renamed history metadata.

The optional v2 archive was read from the existing official trial cache through a
private symlink and rehashed in full. Extracted files and VAD were verified against
the new manifest. Damaged-cache tests affect only generated `/tmp` fixtures.
No huge model is automatically selected by unit tests or normal first launch.

`light/` and `dark-large-fonts/` are final native visual passes with synthetic
history: populated, empty, narrow history, keyboard-opened menu, models normal/
narrow, and all four Settings categories. They also verify exact Unicode/
paragraph Copy and native Export, search/no-results/clear, and Save. Large fonts
are actual GTK Sans 14, verified through GTK settings. All captures are the private
1920×1080 compositor framebuffer. Before/after captures were inspected.

`before/` captures the existing native History/Settings before implementation.
`model-journey/` is the final acceptance journey, including persistent error text,
pinned Cancel, explicit model identity, cancellation and invalid-cache recovery.
There was no prior model-management UI to capture.

## Limits and handoff

- The lab has no registered installed desktop entry; portal shortcut/paste requests
  report that fact. Clipboard Copy/Export and private virtual capture are verified.
  This is not proof of physical-seat shortcuts, real microphone or installation.
- Cancellation is cooperative: network waits retain existing timeout bounds;
  archive extraction checks between entries, and ONNX construction completes before
  the cancellation guard discards the candidate. The previous worker remains alive.
- Network outage/HTTP timeout paths are not newly exercised against the live host;
  local integrity failures and cancellation are exercised. Existing downloader tests
  continue covering bounded writes, digest mismatch and interrupted input.
- Full Orca usability, every compositor, native Windows/Mac, independent reviews,
  runtime-floor validation, installation and user visual approval remain parent gates.
- Complete remaining Mac parity and primary-source v3 findings are in
  `docs/linux-parity-inventory.md`. Formatting, Ask, media, speech and experimental
  engines were not implemented or exposed as fake controls.

The final existing `test_linux_ui.py --record` journey also passes in the private
lab. `preview-handoff/results.json` records exact history/Export, settings
persistence, signal/silence preview, unavailable-input recovery, single-stream
ownership, yielding to dictation and resuming, background search during capture,
nonactivating overlay, pass-through typing and capture-error cleanup. This is
virtual public-fixture audio, not a physical microphone session.

`menu-regression/results.json` records the final existing menu harness passing,
including actual tray dbusmenu dispatch, shortcut activation, draft reuse, named
keyboard menu, hide/show, busy recording state, delivery and capture failure.

Preferences are written to a private temporary file, synced, then renamed over
the prior file. A real read-only-directory test proves a failed save preserves
the previous model preference. API references:
https://doc.rust-lang.org/std/fs/fn.rename.html and
https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all.
