# GNOME recording pill

> Historical evidence and commands use the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). See [the evidence archive](evidence-archive.md) for private reproduction.

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

This is the recording pill step. No screenshot
or clipboard consumption claim is inferred from the controller's test screenshots.
The screenshot method exists only in the private test extension, not production.

Real login/lock/unlock, physical microphone, multi-monitor/fractional scaling,
hotplug, screen reader and Done/recovery Copy/Paste consumption remain acceptance
work. The headless runtime has no lock screen. Only Shell 51 is declared supported.
The lab also has no StatusNotifierWatcher without an indicator extension; GNOME
tray/menu integration or explicit provider setup remains open. Native app windows
and the local CLI remain available. No host install was done for this PR.

## Drawing background capture

The extension's `Capture` call returns one monitor as a PNG, held only in memory. It answers
only the process that owns Koett's GTK application name, refuses during the overview, a modal
dialog, a lock or a monitor change, and hides only Koett's own pill while it captures. A failure
keeps the dictation. On 2026-11-06 `capture-check.py` passed on Shell 51.1 with main merged:
real pixels, an invalid monitor, the overview, a stalled Shell that timed out at 5 s, and the
extension turned off and on.
Evidence: `CrossPlatform/evidence/gnome-capture-20261104/`.

## Dragging

A dock can't use Mutter's own move, so a press outside the pill's buttons calls `DragPill`, and
the extension moves the pill with the pointer until the button comes up. Koett then saves the
place in the file other desktops use. On 2026-11-06 `pill-drag.py` passed on Shell 51.1: the pill
follows the pointer, stays put, is saved and returns after a restart, a press without moving
does nothing, Cancel still works, focus stays on the app and the press never reaches it.

## Drawing

GNOME has no layer-shell, so each monitor gets a transparent GTK window that the extension
makes a dock above everything. It stays hidden from the window list and takes no focus.
GTK's input region didn't work here: Mutter kept the pointer on the old surface. So the
extension's `Input` call switches whether Mutter can hit the window, and hides and shows its
actor in one tick to refresh pointer focus. `WindowAt` names the window under a pin. Both answer
only the process that owns Koett's GTK application name, and `WindowAt` refuses during a lock,
the overview or a modal dialog. The draw key is read from keyboards as elsewhere, because a
dock can't hold keyboard focus, so the focus opt-in doesn't apply on GNOME.

On 2026-11-06 `draw-check.py` passed 28 checks on Shell 51.1, with a virtual keyboard and real
speech: typing and clicks reach the app outside a hold, a Shift stroke draws and is captured, a
Shift click pins and names the window, draw lock, Esc and `koett undo` work, clicks pass right
after drawing, the surface goes when the extension turns off and returns on the next recording.
`capture-check.py`, `pill-check.py` and `pill-return.py` passed with the same extension.
Evidence: `CrossPlatform/evidence/gnome-draw-20261106/`.

Not tested: real hardware, a real keyboard, several monitors, a real lock, and the contact sheet
pasted into an app, because `wl-paste` has no GNOME clipboard path. If the extension turns off
during a recording, drawing stays off until the next recording.

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

Not run on a real lock, because the headless runtime has no lock screen. The extension's
`metadata.json` sets no `session-modes`, and GNOME Shell disables such extensions while the
screen is locked, which closes the pill. Koett now keeps the window, registers the pill again
before showing it, and tries again every 2 s while the extension is off, so the pill returns
without a restart. On 2026-11-06 `pill-return.py` passed on Shell 51.1 by turning the extension
off and on: during a recording, while idle before the next recording, and with a recording
started while it was off. Each time the pill returned as one above-all dock window in its usual
place, focus stayed on the target, and no ordinary window appeared while the extension was off.
Koett started while the extension is off still has no pill until a restart.
