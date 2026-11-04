# GNOME paste-target integration

This optional GNOME Shell 51 extension supplies a fresh focused-window identity
to Koett's existing guarded paste path and per-app image chord selection. It
never activates another window, changes focus, reads the clipboard or handles
keyboard/pointer events. It is the first GNOME integration step; the recording
pill, drawing/input surfaces, pointer context and capture integration remain open.

The extension exports one bounded metadata operation on the Shell session bus.
It compares the caller's Unix PID with the current owner of
`com.olledyberg.Koett`, checks that owner again after resolving credentials, and
returns no target during lock, overview or a Shell modal. Own windows and dock/
desktop windows are excluded. No title, text, image or screenshot is transmitted.
The Rust caller has a 250 ms deadline and a bounded worker queue; failed/busy/late
queries never reuse cached focus. Window IDs are scoped to the unique Shell bus
owner and mapped to tokens that are never recycled during the app process.

## Explicit setup

Copy `koett@olledyberg.com/` to
`~/.local/share/gnome-shell/extensions/koett@olledyberg.com/`, then log out and in
on GNOME Wayland and enable **Koett** in Extensions, or run:

```sh
gnome-extensions enable koett@olledyberg.com
```

Only Shell 51 is declared compatible. Other versions need their own verification.
No installer enables the extension automatically. Without it, Koett retains
Copy/History/manual paste and does not guess the original target. Disabling the
extension removes the bus object immediately; the next focus check fails closed.
`koett paste-target-status` reports `gnome` only when a usable target is returned.

Sources: [GNOME extension lifecycle](https://gjs.guide/extensions/overview/anatomy.html),
[Shell 51.1 session/modal state](https://github.com/GNOME/gnome-shell/blob/51.1/js/ui/main.js),
[Mutter stable window sequence](https://mutter.gnome.org/meta/method.Window.get_stable_sequence.html).

## Recording pill

The optional Shell 51 extension supplies a nonactivating recording pill for
GNOME Wayland, using Koett's existing GTK renderer, timer, waveform, actions and
rescue states. It positions only a registered Koett window as a Mutter dock;
normal Koett windows and other applications are not changed. There is no strut
or reserved desktop space. Exact logical monitor geometry is required.

The GTK application process registers its random window title before mapping.
The extension authenticates the caller PID against the current owner of
`com.olledyberg.Koett`, checks that owner again, and matches the native window's
PID/title before setting its dock type. Mapping is inhibited until that decision.
Dock mouse interaction does not automatically focus the window. Keyboard focus
stays in the editor; the renderer retains its normal pointer input regions.

Only four registrations are allowed. App bus-owner loss and extension disable
remove registered overlay windows; normal app windows stay. A missing extension
retains normal-window recording feedback. After disabling/re-enabling the
extension, restart Koett to recreate its pill. Monitor changes reposition it.


Drawing/capture/pointer context remain open. The same explicitly enabled Shell
extension supplies both focus checks and the recording pill.
