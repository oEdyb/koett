# Native Windows media acceptance

This is genuine Windows 11 x64 in the existing isolated QEMU/KVM guest: four
vCPU, 8 GiB, nice 19. No host app/data/input was used. This is an unsigned debug
source build, not a signed release or physical-hardware claim.

`build-receipt.json` maps every Rust source file and the Cargo manifest to the
locally linked executables, SDK/native toolchain and hashes. Final GUI SHA-256:
`1010c5b3d2ed21d7cdf7e3621b10436e60fd0382d051cc7d8ba3584de98459b2`.
JSON receipts include foreground HWND/PID; screenshots are extracted losslessly
into adjacent PNGs, with paths replacing base64. No binary, model or audio is
committed.

## Passed on final build

- Native backend probes/owned-child lifecycle (2), media model (6), pill actions
  (2), pill state (8), delivery (2), stalled media-worker bound (1), CLI parsing
  (2), logical DPI geometry (1): 24 targeted native unit tests passed. The four
  interactive player tests are ignored in ordinary unit runs and explicitly
  passed separately against Edge (`player-native.json`).
- All six real display combinations: light/dark at 100/150/200%. Media outer
  geometry stays 680×500 logical pixels and edit/button heights 32 logical pixels.
  Settings/theme/175% scale were restored after the tests.
- Focused public Edge editor starts one conversion without opening a media
  window or changing foreground HWND/PID. Its media file uses the actual warm
  Compact backend; dictation History and Last Transcript are unchanged.
- Real Edge InPrivate is refused as context, with a blank manual source and a
  visible unavailable message; no job or file is created.
- Missing ffprobe, silent input and no-audio input produce visible errors.
  Pill Cancel reaps the slow source conversion with no orphan tools. Clipboard
  and dictation Last Transcript survive these failures.
- Media folder denied: visible recovery-file fallback. Recovery folder also
  denied: successful History fallback remains visible. History also read-only:
  raw text survives, new media and CLI Quit are refused, native Quit defaults No,
  core dictation still starts/cancels, and the sticky Copy reminder returns.
  Copy releases the guard; every test fault is restored in `finally`.
- Media shortcut conflict is rejected. A changed binding survives restart; the
  old shortcut is inactive and the new one works. Original config bytes restored.
- Native opt-in pause checkbox plus actual WAV dictation pauses Edge and resumes
  once on cancellation, without focus theft. Explicit native tests also prove
  already-paused no-op, manual play/pause revocation and seek revocation.
- Keyboard Tab reaches the labeled source, Browse, Transcribe and Close with
  Narrator running. An independently decoded segment of actual Narrator audio
  contains the field/button names (`narrator-speech-recheck.json`); this is an
  approximate CPU-ASR receipt, not a human listening or speech-accuracy claim.
- Browse selects a Unicode WAV path; file-picker Cancel preserves the source.
  Native Copy copies the shorter media text and leaves dictation Last unchanged.
- Network physically disconnected through QMP, keys blank in the child
  environment, Edge browser closed and Koett started with a nonexistent WebView2
  runtime override: offline WAV dictation automatically pastes once into Notepad.
  Copy/Paste Last works without duplicate History or foreground change. Koett has
  no loaded browser modules. Windows Search/Widgets WebView2 processes remain;
  the runtime was not uninstalled system-wide.

The standalone console URL/Unicode MP3 cases used build `c24602c` (GUI hash
prefix), whose `media_cli.rs` is unchanged in the final source map. Its receipt
and complete build source map are included. The earlier physical Escape cancel
receipt used `a5e4c8c`; the Escape dispatch path is unchanged. These are explicitly
not represented as executions of the final executable.

## Harness corrections and product fixes

Earlier scaling found a real minimum-size/DPI reentrancy fault; the final native
DPI cell and monitor reconciliation fixed it, then all six strengthened geometry
assertions passed. Earlier storage testing found a hidden reminder after another
dictation and unbounded warning text; the final idle rescue and short warning
passed the combined-storage case. Timed-out media commands now abandon ownership
before any delayed resume can act.

The Notepad harness originally read ValuePattern (empty on the current Notepad)
and clicked a title position that selected an old tab. It now explicitly selects
the unique fixture tab and reads TextPattern. An old lab file-change modal was
dismissed using Keep for all; no product code was changed for that harness error.
The shortcut rejection assertion was corrected to accept the actual "used for
..." message. UIA file-picker steps must wait for the modal to finish and avoid
reused numeric IDs in the dialog. The classic proxy exposes Open as a Pane with
no InvokePattern; the corrected guest harness posts the standard dialog Open
command. `browse-final.json` retains the failed harness attempt, and
`browse-post.json` records the passing retry. The earlier Notepad failure and
corrected tab/TextPattern script are retained. No acceptance assertion was removed.

## Local host checks and shared scope

Rust 1.98 formatting, strict all-target Windows Clippy, local Windows native
linking and Linux all-target Clippy passed. On current main, four Linux runtime
tests passed. The shared History tests passed 23 (one ignored); the same History
implementation is retained after rebasing. The inventory generation/check passed.
No benchmark cohort, Mac build or physical-device gate ran.

The final commit was subsequently rebased onto main `b6ffd02`. Every Windows
and shared Rust source remains byte-identical to the tested executable’s source
map; only upstream Linux sources changed (`rebase-receipt.json`). Supply-chain
outputs were regenerated to preserve main’s new Sparkle inventory/notices.

Shared changes: Windows cfg admission for media commands/adapters, one Windows
namespace feature, a typed media-recovery result preserving the Linux wrapper's
behavior, and metadata from the actual warm CPU worker (also corrects Linux media
metadata with Nemotron selected). No Swift or Linux app source is changed. Main's
Linux media error behavior is retained. Ask remains shelved.

## Limits

Chrome and Firefox are not tested. Physical microphone/USB/Bluetooth/sleep,
30-minute capture, HDR, physical mixed-DPI and native ARM64 are not tested.
The guest uses private checksum-pinned FFmpeg/ffprobe and yt-dlp copies; they are
not in the application archive and redistribution/notices remain a distribution
gate. Optional formatting runtime bundling, signed clean install/upgrade/uninstall,
SmartScreen and signing eligibility are M6 gates. No public release is authorized
or claimed by these receipts.
