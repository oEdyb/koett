# Windows M4 — draw and pin

> Historical evidence and commands use the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). See [the evidence archive](evidence-archive.md) for private reproduction.

M4's software gates passed in the isolated Windows 11 guest. PR #119 is based
on main 6ce37b8 (guest-tested port base d7c8ec4); Windows is not a released platform. Distribution, physical
hardware and the remaining parity milestones are separate gates.

## Behavior

- Recording-scoped physical Raw Input, release-before-arming, chord/other-key
  guards and the shared Mac/Linux gesture fixtures. Injected keys cannot arm
  drawing. Normal typing and idle cross-process clicks remain available.
- One nonactivating Direct2D canvas per display, PMv2 coordinates, mixed DPI,
  negative origins, rotation, display changes, lock and undo. Finished frames
  survive topology changes; unfinished drawing is discarded visibly.
- Gesture-only WGC capture excludes Koett windows. Intended strokes/pins are
  composited offscreen into numbered contact sheets with audio-time anchors.
  Images stay in memory; History keeps final and original speech text.
- Released ink waits 1500 ms and fades for 400 ms. Pins use the Mac's 450 ms
  pulse. Reduced Motion keeps the mark static for its lifetime, then removes it
  without an animated fade. Bounds are 256 frames / 96 MiB retained pixels,
  24 million pixels per capture, and 32 MiB per clipboard bundle.
- Native Settings expose draw enable/key/lock/selected-text and paste choices.
  Text uses paste_shortcut; images use windows:<exe-name>, then windows:*, then
  Ctrl+V. Use default removes an app override. Shortcut-only labels include an
  explicit tab separator so Windows exposes their full UIA names.

## Delivery and recovery

Images are offered with delayed PNG/DIBV5 rendering. Text follows a native read
plus 200 ms, with a 1.5 second fallback when no qualifying read is observed.
Known unrelated readers are ignored. Anonymous OpenClipboard(NULL) reads use
that same processing gap because Edge uses this form.

Text is delayed-rendered too. After its read plus 200 ms, PNG/DIBV5/HTML/Unicode
are restored together only if owner, sequence and marker still match. A newer
copy wins at both stages. Explicit Copy/Paste recovery can offer the bundle
again, without another ASR pass or History write.

Each input stage checks the original HWND/PID, process eligibility, integrity,
clipboard ownership and released modifiers. The final foreground query occurs
**after** clipboard validation, immediately before SendInput. A changed target
or failed delivery produces persistent Not pasted with recovery. SendInput
itself cannot prove consumption in arbitrary applications.

Recovery retains the last result's destination across later cancelled/silent
attempts. Tray focus uses the actual Explorer taskbar ancestor chain, including
Windows 11 composition/input children. It returns to the current editor before
refusing recovery for another window; it never pastes into Koett itself.

## Context and microphone boundaries

UIA reads are bounded, password-safe and rejected when the source window changes.
Own Settings/History cannot pick up context from the browser underneath. URLs
come from the actual browser document/chrome, never page fields impersonating
an address bar. Private indicators are accepted only outside web documents;
Edge's InPrivate browsing label clears title, URL, link, image label and selection.
Unknown privacy/provider fields remain explicit, not a claim of public mode.
YouTube links use actual exposed clock values; percentages never become seconds.

Microphone lookup, opening, device enumeration and preview run in supervised
children of the same executable. Opening has a five-second first-packet deadline;
termination and reader cleanup have bounded waits. Four reader permits remain
held until their readers exit. Pending native controls coalesce requests, so a
stalled driver cannot queue unlimited retries or wedge the hotkey. Preview sends
level pairs, not PCM. The private timeout lab is never used by normal launches.

Windows uses main's selected recording limit and shared 120-minute safety ceiling;
it no longer has a separate ten-minute collector cap. Short fixtures and boundary
arithmetic do not establish long-session or physical-device support.

## Verification

Current receipts, screenshots and foreground HWND/PID are under
[windows-milestone4](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/CrossPlatform/evidence/windows-milestone4):

- [5b7b252 final](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/CrossPlatform/evidence/windows-milestone4/local-5b7b252/README.md):
  six final timing/paste-guard tests, all 15 gesture fixtures and video-time parser;
  actual CPU WAV pin capture with Reduced Motion, physical pill Stop while Shift
  is held, one image/text delivery into the original Edge window, automatic
  four-format restoration, and identical static Transcribing crops 400 ms apart.
- [9bb4c97](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/CrossPlatform/evidence/windows-milestone4/local-9bb4c97/README.md),
  whose tested Rust source matches rebased commit ec90537 (receipt records 5707ac9): 20 native tests; default/named
  opening retries, enumeration/preview faults and normal preview; native UIA
  choices and physical Escape; InPrivate omission, tray recovery/refusal,
  newer-copy protection, cancel/repeat, native recording-limit Save,
  100/150/200%, light/dark and Narrator keyboard navigation.
- [25a8f3b](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/CrossPlatform/evidence/windows-milestone4/local-25a8f3b/README.md):
  virtual display detach/Extend, finished-frame preservation and unfinished-ink
  discard; own-window omission and explicit native editor/terminal/YouTube
  unavailable fields. It also records the private/tray failures fixed later.
- [8b0e469](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/CrossPlatform/evidence/windows-milestone4/local-8b0e469/README.md):
  portrait / negative-coordinate 168/96-DPI ink, lock/undo, native Edge rich paste
  (no synthetic receiver insertion), raw History, silence/image-only behavior,
  pause/cancel/other-key guards and the image-stage newer-copy race.
- Earlier [foundation](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/CrossPlatform/evidence/windows-milestone4/ci-03a7d0a/README.md),
  [dadf83b](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/CrossPlatform/evidence/windows-milestone4/local-dadf83b/README.md),
  [bee18d5](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/CrossPlatform/evidence/windows-milestone4/local-bee18d5/README.md) and
  [5eb985a](https://github.com/oEdyb/koett/blob/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/CrossPlatform/evidence/windows-milestone4/local-5eb985a/README.md)
  receipts retain the initial capture, exclusion and recovery checks.

In total, 42 distinct targeted tests ran on real Windows across the final two
builds; the whole suite was not run. Impacted Linux audio: 11 passed, one hardware
preview test ignored. Local build-tool tests: 22 passed. Windows linking, strict
all-target Clippy, formatting and generated inventory checks passed. No benchmark
cohort or intermediate CI push was used.

The guest's original settings/history/last-result hashes, motion preference and
175% / 1920x1080 + 1280x1024 layout were restored. Narrator was turned off and the
guest shut down. Host pointer, keyboard, clipboard, installed Koett and data were
untouched. Narrator keyboard names/actions were checked; speech quality was not.

## Build and remaining limits

The approved free Microsoft CRT/SDK was installed with xwin 0.10.0 under
~/.cache/koett-windows/msvc-sdk. The pinned ASR-only sherpa-onnx 1.13.5 libraries
include shared-encoder patch
1200f3e6b686eb3a47cc5982c906ba0b0f0b7f9b0e3e69d987556c85c00137f3.
`scripts/windows_local_build.py` uses already installed tools, clang-cl/LLD and
static CRT; it never accepts terms or downloads tools. Optional native-builder
--target/--toolchain arguments preserve native-host defaults.

Builds use four jobs, nice 10, on-disk targets/cache and no memory limit. The guest
uses four vCPUs, 8 GiB and nice 19; startup requires 12 GiB available and 70% CPU
idle. It stays off during builds and outside testing. Source/build receipts
identify private development executables, not signed release packages.

Shared changes are Windows-conditional audio/preview/fixture routing and optional
cross-build tooling; shared gesture/annotation/storage rules are reused. No Mac
app source changed. Linux capture behavior/defaults are unchanged, with the
impacted checks above. Main's recording-limit and shelved-Ask decisions remain.

Edge is the authorized M4 browser gate. Chrome's terms remain pending; Chrome
and Firefox were not installed or tested. Other UIA providers may omit fields.
Physical microphones, USB/Bluetooth/sleep, 30-minute live capture, HDR/cable
hotplug, Qualcomm ARM64 and signed install/update checks remain later gates.
QEMU PCI GPU unplug is unsupported; Windows display detachment was tested.
The prior ARM64 CI failure at 3d7e604 was an upstream ONNX Runtime HTTP 504 after
existing retries; no dependency pin, retry policy or supply-chain policy weakened.

Native API references: [Raw Input](https://learn.microsoft.com/windows/win32/inputdev/using-raw-input),
[WGC interop](https://learn.microsoft.com/windows/win32/api/windows.graphics.capture.interop/nn-windows-graphics-capture-interop-igraphicscaptureiteminterop),
[capture exclusion](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity),
[clipboard ownership](https://learn.microsoft.com/windows/win32/dataxchg/using-the-clipboard),
[HTML byte offsets](https://learn.microsoft.com/windows/win32/dataxchg/html-clipboard-format).
