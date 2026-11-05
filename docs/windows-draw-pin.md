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
No Linux/Mac product code has changed; Cargo additions are Windows namespaces.

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
