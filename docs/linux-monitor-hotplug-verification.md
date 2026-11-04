# Linux monitor changes during dictation

## Behavior

GTK monitor-list changes, monitor invalidation and geometry/scale/connector
notifications invalidate the current drawing layout. The drawing tick handles
this before accepting capture results or scheduling another capture. Pending
strokes, pins, context and captures are removed, including late results; images
already published to the recording remain in order and can still be undone.
No stale ink is painted on replacement screens. Release the draw key before
starting another hold. Audio recording and the existing audio clock continue.

The UI poll rebuilds the drawing surfaces from the current GDK monitors and
recomputes the desktop union used by portal capture. A running niri pointer
lookup keeps its existing input shield until that lookup finishes; surfaces are
not destroyed in the middle of its click/cleanup handshake. The hotplug-during-
pick case still needs compositor-specific acceptance; no new pick mechanism or
universal synthetic-click safety guarantee is claimed here.

When unfinished items are removed, the pill displays “Display changed · redraw
unfinished items” for 12 seconds. The main window explains that completed images
were kept. Ordinary recording feedback, waveform and timer continue. The pill
remaps after display changes, keeping its current monitor when available and
choosing an available monitor otherwise. It never requests keyboard focus.

`koett drawing-status` adds `display_count`, `display_change_pending`,
`display_generation`, and `displays` (connector, logical bounds and desktop
bounds). This exposes whether the live overlay has adopted a changed layout.
Monitor subscriptions disconnect when replaced or dropped. No new dependency,
Windows/shared-engine change, data migration or host installation is included.

## Evidence — 2026-11-04

Rebased on main `16af788` after the KWin identity change (#70). The work-queue
conflict preserves both KWin identity status and the remaining display/input
acceptance gaps. The Rust and private hotplug checks below were rerun on this
combined source.

- Rust suite: 389 passed, 17 ignored, plus the native ABI subprocess check.
- Formatting, clippy with warnings denied, release build and diff check passed.
- MSVC cross-check is blocked by missing Microsoft `lib.exe` on this host.
- Private lab, nested KWin 6.8.2-1.1 with two virtual outputs; copied 110M model,
  isolated silent audio input and a grabbed test-only Shift keyboard.
- Real Wayland/GDK events: scale 1 → 1.25, output disable/re-enable, scale 2 on
  the reconnected output, and removal of the original pill display.
- Counts changed 2 → 1 → 2 → 1 in the same recording. Bounds refreshed from
  1920×1080 to 1536×864 and 960×540; desktop bounds refreshed with each layout.
- One completed image survived a layout change while a second capture was
  deliberately delayed. Its late completion did not resurrect the removed item.
- Drawing continued after resize, on the reconnected output, and after removal
  of the original display. A held draw key did not start a new drawing until
  released. All four completed items remained undoable; the next recording
  started empty. Screenshots were inspected for the visible notice and pill.

The screenshot backend is a delayed `grim` fixture returning a known PNG, not a
real KWin screenshot. These checks prove live monitor events, overlay/input
lifecycle, bookkeeping and crop-target metadata. They do not prove physical
cable hotplug, GPU/driver behavior, real mixed-DPI image alignment, portal grants
or portal image consumption. Those rows remain open. The early 120 ms Shift
click gap is a separate change; capital-letter behavior is not claimed here.
GNOME has no layer-shell overlay in the current implementation; its Shell
integration remains deferred, alongside tiled niri context geometry.

## Reproduce

Evidence lives in `CrossPlatform/evidence/monitor-hotplug-20261104/`. The fixture
requires the local `agent-lab`, KWin, kscreen-doctor, PulseAudio compatibility,
Python/Pillow/dbus, a copied model and the existing grabbed-keyboard helper.
It refuses any runtime other than its named private lab. The input and screenshot
helpers also require its private socket and lab process scope.

Build from `CrossPlatform/` with `CARGO_BUILD_JOBS=4`, `nice -n 10` and a
`systemd-run --user --scope -p MemoryMax=12G` scope, after checking free memory.
Copy the evidence scripts into `CrossPlatform/target/hotplug-sol/` and a copied
model into its `model/` folder. Build the private input helper from `input.c`
using the installed agent-lab's `build/fake-input-{client.h,code.c}` and
`-lwayland-client -lxkbcommon`; do not replace the installed helper.

From the repository root:

```sh
agent-lab start koett-hotplug-sol
systemd-run --user --scope -p MemoryMax=12G nice -n 10 \
  agent-lab run koett-hotplug-sol -- dbus-run-session -- python3 \
  "$PWD/CrossPlatform/target/hotplug-sol/nested.py" "$PWD"
agent-lab stop koett-hotplug-sol --force
```

The nested compositor has a unique configuration directory per run. Host display
configuration, focus, input, clipboard and model files are never used by the test.

Sources: [GTK monitor list and changes](https://docs.gtk.org/gdk4/method.Display.get_monitors.html),
[monitor geometry/scale/invalidation](https://docs.gtk.org/gdk4/class.Monitor.html),
and [upstream layer-shell monitor/remap contract](https://github.com/wmww/gtk4-layer-shell/blob/main/include/gtk4-layer-shell.h).
Pinned Rust bindings: `gdk4`/`gtk4` 0.11.5; no upgrade.
