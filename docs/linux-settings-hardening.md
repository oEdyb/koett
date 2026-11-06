# Linux Settings follow-up — 2026-11-04

> Historical evidence and commands use the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). See [the evidence archive](evidence-archive.md) for private reproduction.

## Crash diagnosis

The Settings reopen failure matches GTK 4.24.1's AT-SPI application-root
`GetChildAtIndex` defect. With a hidden main window and a retired visible
Settings window, an unmatched child lookup retains a hidden/unexported window.
Serializing its NULL accessibility object path crashes GLib. The native GTK
reproduction and source mapping are in
`CrossPlatform/evidence/gtk-accessibility-crash-20261030/REPORT.md`.
Moving panels between windows was not an established cause.

A screen reader is unnecessary: an accessibility client traversing windows can
issue the same request. Without AT-SPI requests this particular handler is not
entered. This does not establish that every possible accessibility crash is fixed.
The private runtime backports upstream GTK commit
`21d3eb76b68d6758122eddaf00b5823f9b2d7f34`; main's launcher/autostart repair
ensures normal startup loads it. Raw development binaries still need the runtime
environment. Actual lab process maps identify the patched library in the receipt.

## Export diagnosis

The baseline installed build reproduced the first-run timeout. A screenshot taken
while the test waited showed the real Export chooser already open, with Name and
Export controls. It was absent from the AT-SPI desktop. The lab starts portal
backends before publishing its accessibility environment; the GTK portal retained
that earlier environment. Replacing only the private GTK portal after obtaining
the lab's AT-SPI address made the chooser accessible and exact Unicode export pass.
The journeys now start that private replacement and reuse the supervisor's
registry instead of starting/stopping competing registries. No product Export
code was changed.

## Changes

- Tab enters the selected Settings page; Shift+Tab wraps to Save. The test sends
  one real key press in either direction and checks the focused action.
- History preserves its current narrow pane while rebuilding rows. Activating
  the selected row still reopens detail after Back.
- Model-switch notices carry an explicit internal marker. Startup, formatting,
  shortcut and other notices cannot become model-switch errors. Windows strips
  the marker before logging.
- Model fixtures are copied into each journey's output. The live model directory
  is never supplied as the writable model root. Keep typing remains removed.

## Verification and limits

Receipts and selected renders: `CrossPlatform/evidence/settings-hardening-20261104/`.
All six model/menu/visual light/dark journeys passed. Selected renders were
inspected. The native GTK fixture passed 20 visible/retired-child cycles and
exited cleanly. Rebased Linux unit tests passed: 303 passed, 13 ignored. Release
build, formatting, Clippy and diff whitespace checks passed locally. Windows native
cross-check was attempted: this Linux host lacks the Windows SDK headers required
by bundled SQLite. CI supplies the native Windows check. macOS code was not changed
by this follow-up; macOS tests were not run here. No independent agent review was
started, as requested.
