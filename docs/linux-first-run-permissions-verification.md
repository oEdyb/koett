# Linux first-visible-launch Permissions

The existing Permissions window opens once when Koett is explicitly opened.
`--background` startup neither opens it nor marks it seen. If Koett started in
the background, a later launcher/`show` request opens Permissions once. Done or
the window close control dismisses it without requiring any grants. Menu →
Permissions and `koett permissions` remain available on subsequent launches.

A private empty `permissions-seen` file alongside settings.json remembers only
that the window was presented. It is created atomically with mode 0600; settings,
model choices and transcript history are unchanged. It does not claim OS access.
Existing installations without the marker see the window on their next explicit
launch, while login startup remains quiet. A failed save is visible and leaves
setup pending; fixing the directory and reopening permits another save attempt.
An invalid marker type is reported as unavailable, never treated as seen.

`koett permissions-status` adds `onboarding: pending|seen|unavailable`.
Keyboard availability and shortcut registration remain live. Microphone checks
still open Audio preview; screen capture still runs only after Check capture.
No automatic microphone preview, screenshot, keyboard setup, desktop focus or
clipboard operation is added by onboarding. Normal dictation remains usable
without completing these optional checks.

## Evidence, 2026-11-04

- 383 Rust tests passed, 17 ignored, plus native ABI subprocess check. The new
  filesystem fixture checks pending/seen state, mode 0600, repeat writes,
  preservation of settings and failure on blocked/invalid marker paths.
- Formatting, clippy with warnings denied, release build and diff check passed.
  MSVC cross-check is blocked on this Linux host by missing Microsoft's `lib.exe`.
- Real release GTK app in private KWin lab `koett-onboarding-sol`, copied 110M
  model, fresh isolated XDG directories and AT-SPI actions. Verified first visible
  opening, Done dismissal, repeated show, restart persistence and manual reopening.
- A fresh background launch kept Permissions hidden and status pending. Explicit
  show then opened it and saved seen. No settings.json was created by onboarding.
- A read-only config directory (with its existing writable instance-lock file)
  displayed the save error, retained pending and saved successfully after repair
  and reopening. Screenshots of normal and failed-save states were inspected.
- A fake `grim` recorded exactly one capture, after the explicit Check capture
  button. The full startup/background/restart/error sequence made no additional
  capture requests. This is a fixture result, not proof of OS permission grants.

Fixture, result JSON and screenshots:
`CrossPlatform/evidence/first-run-permissions-20261104/`.

Real microphone/screen/keyboard grants and revocation, GNOME portal prompts,
compositor activation-token behavior and first-launch packaging acceptance remain
open. No host UI automation or onboarding installation was performed. The host
installation in this batch contains only approved main through PR #66; see
[install receipt](verification-receipts.md#linux-host-install--2026-11-04).

## Reproduce

Check available memory, build from `CrossPlatform/` under four jobs, nice 10 and
MemoryMax=12G, and copy a verified 110M model into
`CrossPlatform/target/onboarding-sol/model` (no model symlinks). From the root:

```sh
agent-lab start koett-onboarding-sol
systemd-run --user --scope -p MemoryMax=12G nice -n 10 \
  agent-lab run koett-onboarding-sol -- python3 \
  "$PWD/CrossPlatform/evidence/first-run-permissions-20261104/check.py" "$PWD"
agent-lab stop koett-onboarding-sol
```

The fixture refuses other runtimes. It recreates only its named scenario
subdirectories below `target/onboarding-sol`, keeps its copied model, uses no
virtual keyboard input, and exits the private app through guarded Quit.
