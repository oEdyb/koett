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
   URL/private-mode/YouTube fields are tested separately in available browsers;
   missing context remains absent. No synthetic Copy or guessed privacy signal.
   Chrome itself remains untested: its terms were not accepted. Edge is the
   authorized Chromium browser for M4 paste verification; Chrome is not a gate.
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

## Implementation

Windows uses recording-scoped physical Raw Input, nonactivating Direct2D canvases,
bounded WGC captures, the existing Mac/Linux gesture rules, numbered contact
sheets and audio-time anchors. Draw lock waits for released modifiers; left drag
paints, right-click/Ctrl+Z undoes, and Escape unlocks before cancelling.

Pin context uses bounded UIA reads, with password omission, explicit privacy
signals and visible missing context. Browser URLs come from the actual browser
document or browser chrome, never a page input that impersonates the address bar.
No title-to-URL or seek-percentage-to-seconds inference is allowed.

Image paste offers delayed image formats first, then text after a native read
plus 200 ms. Known unrelated readers are ignored; anonymous OpenClipboard(NULL)
reads use the same gap, because Edge uses that form. With no qualifying read,
the fallback is 1.5 seconds. Every stage rechecks HWND/PID, clipboard owner,
sequence and marker, integrity level and released modifiers. Newer copies win.
Text is also offered with delayed rendering. After its read plus 200 ms, the
combined PNG/DIBV5/HTML/text offer is restored only while ownership, sequence
and marker remain unchanged. Repeat paste does not write history again.
Copy offers the same four formats. Native clipboard round trips passed; the
latest automatic restoration still needs the browser journey.
Raw speech remains available in History when final text contains annotations.

Settings expose draw enable/key/lock/selected-text controls and native per-app
image-paste shortcut menus. Text uses paste_shortcut; image chords use the
windows:<exe-name> override, then windows:*, then Ctrl+V. Images remain in memory
and are not written to history.

Microphone lookup/opening runs in a supervised child of the same executable.
The five-second ready deadline includes the first packet; termination and reader
cleanup have bounded waits. Stalled readers retain one of four permits, reserved
before spawning, to prevent runaway retries. Error permits a later hotkey retry. Windows device enumeration and preview
also use supervised children with five-second opening deadlines. Preview sends
only level pairs, never PCM, and its bounded cleanup releases recording priority.
The 1249196 guest passed virtual-HDA preview start/stop, forced enumeration
timeout, two preview timeout/retry rounds with duplicate-command suppression,
and four default/named-input hotkey timeout/retry rounds. No worker remained.
The private timeout lab is not enabled in normal launches.

## Local build

The Microsoft build-tools terms were explicitly approved on 2026-11-05.
xwin 0.10.0 installed the free CRT/SDK under ~/.cache/koett-windows/msvc-sdk.
The pinned native ASR libraries, including shared encoder patch
1200f3e6b686eb3a47cc5982c906ba0b0f0b7f9b0e3e69d987556c85c00137f3,
link locally with clang-cl/LLD and the static CRT. No payment or sudo was needed.

scripts/windows_local_build.py uses already installed tools and never downloads
or accepts terms. The optional build_asr_native.py --target/--toolchain path
preserves native-host defaults. Builds use four jobs, nice 10 and on-disk cache
and target directories, without a memory limit. No intermediate CI pushes are
permitted before M4 is ready for merge.

Shared changes are Windows-conditional audio counters/worker routing and optional
cross-build tooling; the test-only Windows logic wrapper has no production effect.
Mac/Linux capture callbacks, defaults and app behavior remain unchanged. Shared
build-script tests must accompany any final push. Mac source is untouched.

## Real Windows evidence

- [Initial foundation](../CrossPlatform/evidence/windows-milestone4/ci-03a7d0a/README.md):
  clipboard formats/newer-copy protection, WGC exclusion and physical draw key.
- [Local dadf83b](../CrossPlatform/evidence/windows-milestone4/local-dadf83b/README.md):
  14 targeted tests, WAV capture, canvas/pins, three-frame contact sheet, pill
  Stop while holding Shift, one insertion into the original Notepad and Copy.
  Notepad cannot receive an image; visible recovery is expected. The early test
  harness lost the exit code; it records the successful test output honestly.
- [Local bee18d5](../CrossPlatform/evidence/windows-milestone4/local-bee18d5/README.md):
  default/named microphone timeout followed by hotkey retry, each around five
  seconds with no orphan worker; Edge selection/password and staged paste.
- [Local 5eb985a](../CrossPlatform/evidence/windows-milestone4/local-5eb985a/README.md):
  16 targeted tests with exit 0; Edge image then text 248 ms apart and stable
  HWND/PID; two working signed VirtIO display adapters; 1080p Settings at 175%.
  This is a partial M4 receipt.
- [Local 8b0e469](../CrossPlatform/evidence/windows-milestone4/local-8b0e469/README.md):
  tray recovery, changed-target rejection, mixed DPI/portrait/negative-coordinate
  ink, lock/undo, native rich paste, newer-copy race, raw speech, silence and
  image-only output, other-key release, pause and cancel. Its native menu
  accessibility failure was traced to Windows parsing shortcut-only labels as
  accelerators. A native guest probe proved an explicit tab separator keeps
  the accessible names; the rebuilt choice journey is still pending. PCI GPU unplug is unsupported by this QEMU device.

- [Local 1249196](../CrossPlatform/evidence/windows-milestone4/local-1249196/receipt.json):
  17 native tests exited 0, including delayed-text combined-offer restoration
  and newer-copy protection; virtual preview and forced microphone stalls
  passed with bounded cleanup and stable external foreground HWND/PID.

Original guest settings, history and last transcript are backed up and restored
with hash checks after testing. Only the guest receives input. It runs at nice
19 with four vCPUs and 8 GiB; the controller requires 12 GiB available and at least
70% host CPU idle. The guest stays off during builds and outside testing.

## Remaining gates

- Run automatic Edge image/text completion and confirm the combined clipboard
  restores without pressing Copy; repeat with a newer copy after text consumption.
- Retest changed-app tray restoration and retained recovery destination across
  a cancelled or silent attempt in another editor.
- Invoke rebuilt paste choices through UIA, including per-app inheritance;
  finish private/native/terminal/YouTube context cases with honest missing fields.
- Detach/reattach the virtual secondary display through Windows; finished frames
  must survive while unfinished ink is discarded. PCI GPU unplug is unsupported.
- Exercise new controls at 100/150/200%, live light/dark, reduced motion and
  Narrator/keyboard; inspect screenshots before calling accessibility complete.

Already passed in the receipts: supervised virtual preview/enumeration timeout,
repeated default/named microphone hotkeys, native delayed clipboard round trips,
native rich Edge paste, image/text read gap, image-stage newer-copy race, raw
History, silent clipboard preservation, image-only output, held-key pause/cancel,
other-key release, and portrait/negative-coordinate mixed-DPI ink.

Chrome was not installed or tested: its terms remain pending. Edge is the
authorized Chromium browser for M4 paste checks; Chrome does not block M4.
Physical microphone/display/HDR/hotplug and long live capture remain distinct
hardware gates, not claims established by this virtual guest.

The 3d7e604 ARM64 CI failure was an upstream ONNX Runtime HTTP 504 after existing
retries; the pinned URL returned 200 on recheck. No pin or policy was weakened.
No large benchmark is part of M4. M4 remains draft and incomplete.
