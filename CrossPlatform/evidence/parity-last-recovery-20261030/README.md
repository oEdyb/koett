# Durable Last, volatile recovery and guarded Quit

Implemented on shared `feat/linux-ui-history`, preserving the existing desktop,
microphone, settings-merge and other feature work. No install, commit, push,
live preferences, user transcripts, host input/audio/clipboard or cloud calls.
No appearance approval is claimed.

## Behavior

- Nonempty dictation replaces exact UTF-8 Last in memory and atomically on disk,
  mode 0600. Empty cleaned output preserves prior Last and retains its raw record.
  Delivery still happens before storage. Last restores on a background worker.
- Completed Ask questions update in-memory Last. The durable Last file remains
  ordinary dictation. Media result/delivery caches are separate; failed media
  saves use shared history/recovery without pretending SQLite is a media file.
- Every failed primary+fallback save retains the full stable-ID record in memory.
  Retry reuses that ID and SQLite's collision checks. Multiple pending records
  survive later completions. Copy Unsaved sorts timestamp/ID and uses raw text
  when final text is empty. Neither operation runs ASR.
- History saved with Last failed yields a distinct saved-history warning. Pending
  memory-only records have persistent Copy Unsaved and Retry Save controls.
- Ordinary Quit retries pending records and refuses exit if saving still fails.
  Native confirmation offers Keep open, Copy Unsaved, Retry Save and explicit
  Quit Anyway. Capture/processing refuses ordinary exit. Existing worker shutdown
  owns cancellation of Ask/media/model/preview work.
- Shared native menu/tray/CLI operations: `copy-last`, `paste-last`, `copy-unsaved`,
  `retry-unsaved`, `recovery-status`, `media-transcripts`, `failed-transcripts`,
  `failed-recordings`, `quit`; discover with `recovery-help`.
  Status contains counts, availability and Last source, never transcript text.
  CLI JSON includes success or a fixable error; rejected operations exit nonzero.
- Paste Last uses clipboard-only delivery when external focus cannot be proven,
  with an explicit manual-paste notice. It never activates another application.
- Folder actions use GIO's default opener and actual XDG directories. The fixture
  configures only its private MIME default and opens three real Dolphin windows.

## Verification

`history-tests.log`: real isolated SQLite, raw/final/model/ID preservation,
two distinct failed records, repeated retry, pending-ID collision, exact Last,
0600, permission-denied atomic replacement preserving the old file, and separate
history-saved/Last-failed reporting. Existing history tests remain intact.

`linux-tests.log`: affected Linux tests, including all eight tray recovery/status/folder
operation dispatches and existing delivery/storage separation checks.
`audio-tests.log`: affected input/recording ownership tests.
`media-tests.log`: affected media URL/storage tests.
`preview-strict.log`: explicit ignored hardware test run inside the sole private
`koett-last-recovery` lab; a missing selected device errors instead of falling back.

`clippy.log`, `fmt.log`, `release.log`, `msrv.log`, `windows.log`, `ruff.log`:
strict all-target Clippy; formatting; locked release; Rust 1.92 source check;
Windows source check using installed Rust 1.98 and `DOCS_RS=1`,
`LIBSQLITE3_SYS_USE_PKG_CONFIG=1`, `PKG_CONFIG_ALLOW_CROSS=1`; Ruff 0.17.2.
Python fixture also passes `py_compile`. Native link commands use
`SHERPA_ONNX_LIB_DIR=/tmp/koett-native-host-abi0/lib`.

The native fixture reuses the existing model harness preparation and helpers.
It uses private XDG directories under `/tmp`, synthetic Last text and cached
public model test audio routed exclusively through `agent_lab_audio`.
It checks exact Copy across actual process restarts; clipboard fallback without
focus activation; missing stored microphone ID to real CPAL capture, visible
warning and persisted replacement; one successful ASR row; two failed saves;
exact Copy Unsaved through CLI and native button; failed retry; Quit refusal;
keyboard/narrow confirmation; native Retry Save and repeated CLI retry without
duplicates; no successful-ASR WAV retention; real private folder windows;
explicit private-fixture Quit Anyway; and success/error CLI exit codes.
The final Recovery Status action is exercised and captured in both themes
on the latest release binary, including restored Last source and exact Copy.

Reproduce from repository root after starting the named lab:

```sh
agent-lab run koett-last-recovery -- python3 \
  CrossPlatform/evidence/parity-last-recovery-20261030/native-fixture.py \
  --lab koett-last-recovery \
  --binary /home/olle/dev/koett/CrossPlatform/target/release/koett \
  --output /tmp/koett-last-recovery-new --theme light --renderer cairo
```

Selected light/dark, save-failure, menu, microphone-warning, keyboard/narrow
protection and folder screenshots were inspected. Raw fixtures stay in `/tmp`.
Full-suite execution and independent final review belong to the parent.

## Remaining parity

Parent review found a whitespace-only failed save could overwrite Last and a
queued capture start could bypass the controller Quit guard. The fixes preserve
nonblank Last ownership and mark capture pending before worker acknowledgement.
Ask cancellation/error releases that pending guard. Paste Last now uses the
session's clipboard tool on X11 as well as Wayland, retaining manual-paste safety.
Focused recovery tests passed (3), queued-capture classification passed (1).
Native queued-command acceptance remains part of the next integration pass.

All three global shortcuts; full menus including microphone, mode, shortcuts,
setup and version; media HLS; focused-browser detection; final UI and packaging
acceptance. Physical microphone hotplug and unrestricted host integration are
outside this private fixture. No Swift, vendor, dependency, default-model,
Raw-default or speech-off change was made.
