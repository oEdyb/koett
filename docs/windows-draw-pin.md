# Windows M4 — draw and pin

Base: main at 8344c545503ddf76f54a62023993f74a1f8020d3, after M3 #113.
Work is on windows-draw-pin. M4 is not complete or released.

## Acceptance

1. Physical draw-key input is recording-scoped, must be seen up first, and is
   spoiled by other keys. Injected input cannot arm drawing. Ordinary capitals,
   chords and cross-process clicks keep working without keyboard focus theft.
2. Mac movement/pin/double-click/drag/hold fixtures pass. Undo, pause, cancel and
   pill-click stop preserve session identity, audio timing and the original target.
3. One nonactivating overlay per display handles real DPI, negative coordinates,
   rotation and hotplug. WGC excludes Koett windows and captures only on gestures.
4. Bounded in-memory frames become the numbered contact sheet and timed bullets.
   Images are not stored in history. Capture failures stay visible and bounded.
5. Image-first delivery uses observed target read plus processing time, with a
   bounded unknown-reader fallback. Every stage rechecks target and ownership;
   newer copies win. Repeat Copy/Paste offers PNG, DIBV5, HTML and exact text.
6. UIA context is explicit, bounded, password-safe and stale-result-safe. Browser
   URL/private-mode/YouTube fields are tested separately in Edge/Chrome/Firefox;
   missing context remains absent. No synthetic Copy or guessed privacy signal.
7. Native guest evidence includes screenshots, foreground HWND/PID and clipboard
   formats. Physical mixed-DPI/HDR/hotplug limits stay distinct from guest proof.

Expected product changes: Windows modules, Windows feature flags in Cargo and
their generated supply-chain outputs. Existing platform-neutral annotation,
pin sanitization and gesture rules are reused. Shared/Linux changes, if needed,
must be identified and checked separately. Mac app code remains native Swift.

## Native boundaries

- [Raw Input](https://learn.microsoft.com/en-us/windows/win32/inputdev/using-raw-input):
  INPUTSINK only during recording; no NOLEGACY and no retained typed-key history.
- [WGC monitor/window interop](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nn-windows-graphics-capture-interop-igraphicscaptureiteminterop)
  and [capture exclusion](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity).
- [Clipboard ownership/rendering](https://learn.microsoft.com/en-us/windows/win32/dataxchg/using-the-clipboard)
  and [UTF-8 HTML byte offsets](https://learn.microsoft.com/en-us/windows/win32/dataxchg/html-clipboard-format).
- [WIC encoding](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-creating-encoder):
  native in-memory PNG; no image codec dependency upgrade.

No large benchmark is part of M4. Focused deterministic fixtures and real guest
journeys establish behavior; they do not establish broad hardware compatibility.

## First implementation checkpoint

Windows-only foundations now compile: recording-scoped Raw Input with release/
chord/idle guards, bounded asynchronous WGC, explicit own-window exclusion,
in-memory WIC PNG/DIBV5/UTF-8 HTML/text, and sequence/owner/marker protection.
Plain-text recovery uses the same allocation-before-empty clipboard transaction.
Linux 431 tests (22 ignored), Clippy/release and Windows metadata Clippy passed.
New Windows native unit/guest checks still need the linked executable.

The drawing canvas, contact-sheet/linking, staged reader-aware paste and UIA
context matrix are still being implemented. No finished drawing UI is claimed.
Shared audio.rs and audio/fixture.rs now contain Windows-only sample counters
for drawing timestamps. They count mono frames, skip pauses and stop with the
last fixture frame. Linux/Mac callbacks and defaults are unchanged; Cargo
additions are Windows namespaces.

The first native Windows CI run reached 236 passing tests and caught an offer
sequence captured before CloseClipboard. The follow-up records the published
sequence after close, retaining owner/marker/sequence checks and the test that
rejects a newer user copy. This correction still needs a native rerun; the
guest has not run M4 yet. Windows metadata checking does not prove Win32 runtime
behavior.

Private `--desktop-lab` messages, to KoettOwnerWindow:

- 0x8057, wParam=0: publish the four-format 2×2 image fixture; 1: stage text only
  if the prior offer remains owned; 2: repeat the identical bundle explicitly.
- 0x8058: one bounded WGC monitor capture into the bundle, preserving a newer
  clipboard copy. Koett windows must all accept exclusion before capture begins.
- 0x8059, wParam=1: observe the Shift draw key while the lab is recording;
  wParam=0 releases registration. The hidden owner's caption reports booleans.
  No raw key names or histories are recorded. Stop also releases registration.

These messages are ignored by normal product launches. During exclusion tests,
use the private QEMU framebuffer for UI evidence: normal capture APIs correctly
omit Koett's excluded windows. Image capture remains memory-only in the app.

## Canvas and delivery implementation

The next checkpoint adds per-display nonactivating layered Direct2D canvases,
the existing Mac/Linux gesture fixtures, bounded WGC frame collection, numbered
contact sheets, captured-audio anchors and image-first delayed rendering.
The text stage waits for a target-PID read plus 200 ms, or a 1.5-second unknown-
reader fallback. Ownership and target checks still guard the text stage.
Settings and tray expose Draw while dictating, and the tray exposes Undo.
These paths require native guest verification; context reading, complete repeat
paste, draw-lock/shortcut controls and timeline integration are still underway.

Windows physical microphone lookup/opening now runs in a supervised child of
the same executable, with a five-second ready deadline covering the first audio
packet. Timeout requests termination and bounds process/reader cleanup before returning an error. The app
rejects duplicate starts while opening, and a later toggle may retry. Audio IPC
is memory-only with bounded packets, finite samples and a ten-minute capture
limit. Pause discards packets before metering, timing or ASR. This replaces no
Mac/Linux capture behavior. Shared audio.rs/runtime.rs additions are Windows-
conditional; a small resolver extraction leaves the Linux algorithm intact.
`--microphone-open-timeout-lab` injects a stalled child for the private guest
journey; it is never active in normal launches. Windows tests cover process
timeout/termination followed by a responsive worker and corrupt IPC rejection.

## Local work after the last push

No further push is permitted until M4 is ready for merge. The working changes
add bounded native pin reading and visible missing-field status; no title-to-URL
or seek-percentage-to-seconds inference is allowed. Private mode is never assumed
false. Password selection is omitted. Capture results remain owned by their
session; undo/cancel/display changes drop unfinished work. Multi-display holds
use one bounded serial capture worker. The pill has a nonactivating Stop action,
numbered timeline bands, repeat Paste and four-format Copy. Repeat delivery does
not append history again. Each chord rechecks the clipboard offer after the
modifier wait. Image chords use `image_paste_shortcuts["windows:<exe-name>"]`,
then `windows:*`, then Ctrl+V; text uses the existing `paste_shortcut` preference.
Native per-app shortcut editing still needs completion and a guest check.

Drawing settings expose key choice, lock shortcut and selection opt-in. Lock
waits for shortcut modifiers to be released; left drags draw, right-click/Ctrl+Z
undo, Escape unlocks before it can cancel. Koett's pill remains a pointer target
inside the shield. These integration paths are only compile-checked so far.
The expanded Settings layout must be inspected at every required scale before
calling it finished.

Windows microphone timeout cleanup is bounded too: neither process termination
nor reader cleanup can introduce a new unbounded join on the command thread.
Stalled readers retain one of four permits until they exit, preventing runaway
retries. The forwarding child does not retain a second full audio recording.
The actual driver and hotkey journey still requires the rebuilt guest binary.

Focused local checks passed: 15 existing Mac/Linux gesture fixtures; nine Windows
meter/paste-timing/pin-policy tests made runnable in non-Windows test builds;
seven shared pin-context fixtures; strict Windows library/test/desktop metadata
Clippy. No full suite or release rebuild was repeated after the workflow change.
The test-only lib.rs module has no effect on any production platform.

Real guest foundation evidence is in
`CrossPlatform/evidence/windows-milestone4/ci-03a7d0a/README.md`. The next boot was
refused by the existing host-idle guard; the guest and transfer server are off.
Neither host nor guest has the Microsoft link libraries. Local Windows linking
therefore awaits the requested licence decision for xwin 0.10.0's Microsoft
CRT/SDK download, entirely under `~/.cache/koett-windows/`. The exact prompt is
[Microsoft's build-tools terms](https://visualstudio.microsoft.com/license-terms/mlt031519/).
No licence has been accepted, no SDK installed and no money spent. The already-
produced `3d7e604` artifact is cached; it does not contain the newer local fixes.
M4 remains draft/incomplete. No release source or packaging has changed.
