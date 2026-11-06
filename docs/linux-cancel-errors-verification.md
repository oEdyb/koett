# Linux Escape and pill errors

> Historical evidence and commands use the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). See [the evidence archive](evidence-archive.md) for private reproduction.

2026-11-04. Independent row-2 change, rebased on main `2257ab8`; no dependency on
the Linux transcription CLI PR. No installation or user desktop input.

## Behavior and limits

Wayland now observes Escape through Koett's existing read-only evdev keyboard
state sampler during dictation. Fresh unmodified Escape calls the same engine
cancellation path as the pill. A held key at capture start, key repeats and
modifier chords do not cancel. Release is required before another press counts.
The monitor closes its handles and resets when recording ends; it is independent
of drawing being enabled. Polling occurs every 16 ms; presses entirely between
two polls can be missed. Hotplug discovery retries once per second.

This does not grab keys or focus: Escape also reaches the focused application.
It works across Wayland compositors when keyboard read access is available.
Without that access, the pill Cancel and `koett cancel` remain the explicit
fallback; Settings → Shortcuts explains this and points to existing keyboard
access setup under Draw on screen. No permissions are granted automatically.
Do not bind bare Escape permanently and claim it only affects recording.

The [GlobalShortcuts portal API](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)
has session-scoped bindings, may display a permission dialog when binding, and
provides no per-shortcut enable/disable call. The
[kernel evdev interface](https://docs.kernel.org/input/input.html#evdev)
is the existing compositor-independent access path. These constraints do not
establish an OS-wide impossibility: a compositor extension can offer more.

Existing fatal engine failures already reached the five-second pill. This change
also routes explicit startup/recovery/delivery failures, including control CLI
recovery errors, to that surface. Ordinary notices stay ordinary notices;
recording state is preserved and persistent transcript rescue has priority.
Copy/paste/save failures receive short labels; full details stay in the window.

## Fresh verification

- `cargo fmt --check`: passed.
- `cargo test --locked`: 372 passed, 14 ignored in the library suite; isolated
  native ABI test also passed. Ignored tests do not establish acceptance.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo build --release --locked`: passed.
- `git diff --check`: passed.
- Windows cross-check failed in bundled SQLite: missing MSVC `lib.exe` on Linux. No Windows source or shared
  engine code changed. Swift/build packaging checks do not apply to this diff.

Builds used `systemd-run --user --scope -p MemoryMax=12G nice -n 10 env
CARGO_BUILD_JOBS=4`; all output stayed under the worktree's `CrossPlatform/target`.

Private KWin Wayland lab: `koett-row2-sol`. Copied 110M model, isolated XDG data,
private silent microphone, and a grabbed virtual keyboard selected exclusively
by `KOETT_TEST_DRAW_KEYBOARD=Koett row2 sol grabbed`. The virtual keyboard was
removed and the app stopped after the check. No host keys were injected.

Runtime checks passed: idle Escape ignored; control CLI recovery error appeared
on the pill and expired after five seconds; Escape held when recording began
was ignored; Ctrl+Escape was ignored, including releasing Ctrl first; a fresh
Escape cancelled live capture and returned to Ready. No last transcript or
failed audio file was created. The error also appeared with the main window hidden.
[Screenshots](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/CrossPlatform/evidence/cancel-errors-20261104) of the background
error and cancelled state were inspected. These checks do not prove every physical keyboard/compositor,
sub-16-ms taps, denied-permission behavior on a clean login, or GNOME's existing
lack of a layer-shell pill backend. That desktop still uses window feedback.

Remaining Linux rows: pin context, per-app paste/permissions, acoustic dictionary
bias and spelling around formatting, then remaining partial audit rows.
