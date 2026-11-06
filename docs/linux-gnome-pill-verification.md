# GNOME recording pill

## Result

The explicitly enabled Shell 51 integration adds Koett's existing native GTK pill
on GNOME Wayland. Window caller PID and current GTK application bus owner are
checked before accepting a random-title registration. Only the matching native
normal window is changed to a Mutter dock before mapping, so initial show and
mouse clicks do not activate it. It is excluded from the window list, sticky,
placed by exact logical monitor geometry and raised after Mutter adds it to the
window stack. No desktop space is reserved. All existing waveform, timer, hover
and rescue rendering remains shared with layer-shell.

Absent integration keeps normal-window feedback. Extension disable closes only
registered pills; Koett stops updating that closed surface, keeps the recording
and accepts explicit Cancel. After re-enabling, restart Koett to recreate the
pill. Normal windows, other applications, focus and clipboard are not changed.
Only four registrations are allowed, and app bus-owner loss clears them.

## Native verification — 2026-11-04

Base: main `441c302`, after the focus-identity PR merged. Private Shell/Mutter
51.1 with a real virtual monitor inside `koett-gnome-sol`; isolated bus/runtime,
unprivileged read-only native-package overlay, copied ASR model and isolated
microphone source. No system packages or host extension were installed.

- A real fullscreen GTK editor stayed focused across two recording cycles.
  Both native pill windows had dock type, above state and exact 300×48 bounds
  at (490,728) on a 1280×800 monitor, with the expected 24 px bottom margin.
- Real private pointer motion revealed Cancel/Pause/History. Cancel worked in
  both cycles; the editor received zero clicks. Screenshots of the strip and
  hover actions were inspected. No focus-changing repair was needed.
- The editor received seven actual capital `A`s: before recording, during/after
  each cycle, after extension disable, and after a fresh launch without the extension.
  Final text was `AAAAAAA`.
- An unrelated D-Bus process could not register a pill while Koett owned its
  application name. Disabling the extension removed the pill while recording
  continued; CLI Cancel still completed it and typing stayed in the editor.
  A fresh process without the extension also recorded/cancelled without a pill
  and kept typing in the editor.
- The final Shell session logged no Mutter criticals or JavaScript errors.
  The first prototype raised before stacking; using the native shown event
  fixed that warning. The test helper now verifies pointer coordinates, avoiding
  a large relative motion that had landed on the top screen edge.

Rust suite: 398 passed, 19 ignored, plus native ABI subprocess check. Formatting,
all-target clippy with warnings denied, release build and diff check passed.
MSVC check is blocked by missing Microsoft `lib.exe` on Linux. Independent review
is pending. Evidence: `CrossPlatform/evidence/gnome-pill-20261104/`.

## Open work

This is the recording pill step. GNOME drawing surfaces, Shift input/gestures,
capture exclusion and pointer context still need implementation. No screenshot
or clipboard consumption claim is inferred from the controller's test screenshots.
The screenshot method exists only in the private test extension, not production.

Real login/lock/unlock, physical microphone, multi-monitor/fractional scaling,
hotplug, screen reader and Done/recovery Copy/Paste consumption remain acceptance
work. The headless runtime has no lock screen. Only Shell 51 is declared supported.
The lab also has no StatusNotifierWatcher without an indicator extension; GNOME
tray/menu integration or explicit provider setup remains open. Native app windows
and the local CLI remain available. No host install was done for this PR.

## Reproduce

Use the staged runtime/package-hash procedure in `linux-gnome-focus-verification.md`.
Copy the helpers to `CrossPlatform/target/gnome-sol/`, adjusting checkout paths
and creating `runtime/pill`. Install the production extension only under the
named lab's XDG data directory. Install `test-controller.js` as a separate private
`koett-test-sol@local` extension (metadata declares Shell 51 and that UUID).
This controller is lab-only: it captures test artifacts and reports private
window/pointer state. Never install it on a user session.

Build from `CrossPlatform/` with four jobs, nice 10 and a 12 GiB user scope.
Start the named private lab/runtime, then run `pill-check.py` in the same bounded
scope. Its exact private runtime and Pulse server guards reject host execution.
It launches a real fullscreen editor, checks the production pill, injects input
only through the private Mutter RemoteDesktop session, and quits its apps.
Stop only that named lab after collecting evidence.

## Screen lock

Not run on a real lock. The extension's `metadata.json` sets no `session-modes`, and GNOME
Shell disables such extensions while the screen is locked. Disabling the extension closes
the pill, and the native run above confirmed Koett needs a restart to get it back. So a
screen lock very likely loses the pill until Koett restarts, while recording itself
continues. Checking this needs the private GNOME session rebuilt, because its start steps
were not saved.
