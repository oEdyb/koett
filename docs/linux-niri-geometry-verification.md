# Optional niri pin geometry — native verification

## Outcome

The optional pinned niri adapter provides tiled-window coordinates to Koett's
existing context reader. Stock niri 26.04 still omits them. This implements a
verified alternative; it does not claim stock-niri parity or install/replace a
compositor. The host Koett batch installed separately is main `3b4785f`, without
this adapter. GNOME integration remains next.

The adapter adds `koett_geometry_v1` only to a `PickWindow` reply. It captures
pointer position, window-buffer position, visible geometry and full surface-tree
bounds from the compositor's input hit test before releasing the pick grab.
Activation-only hits carry no geometry. Normal window lists and event streams
remain unchanged; no viewport or column-position reconstruction is used.

Koett matches the accessible frame's reported size against the visible or full
buffer bounds. GTK and Chromium expose different origins; the verified size
selects the conversion. A mismatch, invalid rectangle, border hit, nonfinite
coordinate or pointer motion beyond one logical pixel yields no hit.
This stricter metadata guard keeps the existing input hole unchanged and avoids
reading a different overlapping window after pointer drift. Duplicate window
titles still omit exact context. Existing selection/password,
page URL/YouTube-time, deadlines, focus restoration and pick shields remain.

## Fresh evidence — 2026-11-04

- Koett base: main `3b4785f` with #70/#71/#72. Rust: 396 passed, 18 ignored,
  plus native ABI subprocess check. The ignored native test was then run
  explicitly for 10 adapter cases and one stock-niri case.
- Formatting, clippy with warnings denied, release build and diff check passed.
  MSVC check remains blocked by missing Microsoft `lib.exe` on Linux.
- Patched niri source `8ed0da44d974c32c6877d2f4630c314da0717ecb` (v26.04):
  minimal nested compositor built; niri-ipc's two tests and one doctest passed.
  The patch also passed a reverse-application check against the built source.
- Private KWin lab with nested niri, two native GTK windows and a native Wayland
  Chrome profile. Every picked browser window was tiled and still reported
  `tile_pos_in_workspace_view: null`; the added snapshot supplied coordinates.
- GTK's actual pointer coordinates matched the snapshot after its measured
  14×12 decoration offset. Its accessible frame was 616×768 while its buffer
  was 644×797. The native image returned `Native image fixture`.
- Chrome's frame/buffer was 648×810 around a 616×768 visible window. The
  production Rust reader returned distinct HTTPS links and image labels from
  the left and right halves of the real local page. For the left half, visual
  point (154,384) mapped to the actual accessible point (170,394).
- Centering another column moved the browser's visible origin from (648,16)
  to (-300,16). A pin in the exposed part still returned the right-hand link
  and image, proving the lookup does not assume a fixed or positive origin.
- Missing adapter fields, excessive pointer motion, mismatched frame sizes and
  three real windows sharing one title all returned no link/image.
- Unmodified installed niri 26.04 was also launched privately. Its real reply
  omitted the adapter field; Koett returned no guessed pointer metadata.
- Native GTK label links exposed no URI and retained the clean fallback. The
  browser cases prove actual URI reads; GTK's unavailable URI is not fabricated.
  A screenshot of the native browser/GTK fixtures was inspected.

Evidence: `CrossPlatform/evidence/niri-geometry-20261104/`. It includes actual
picked-window replies, GTK/Chrome frame coordinates, stock and adapter results,
a screenshot, source/binary/patch hashes and the reproducible scripts. No host
pointer, keyboard, clipboard, browser profile or model was used by the lab.

## Limits that remain open

This is a component test of actual niri picking, coordinate translation and
Koett's production AT-SPI reader. It does not revalidate the complete overlay
input-shield/capture/paste session, physical/multiple monitors, fractional DPI,
continuous animations, overview, portal grants or every browser/toolkit.
The GTK/native/browser windows and the niri viewport were real; only the moved,
missing and mismatched reply cases replay modified copies for rejection tests.

The built compositor omits default D-Bus/systemd/screencast features for the
private nested test. Normal-session packaging, distribution support and adapter
deployment need separate acceptance. No automatic source replacement, login
change or system package modification is included. Stock-niri fallback remains
image/app/title/page/selection as available; it never invents a pointer hit.
The installed host also retains the separately documented pre-sample Shift race.

## Reproduce

Check free memory. Build the adapter with
`CrossPlatform/integrations/niri/build-adapter.sh` on a clean exact source pin,
and run Koett's Rust gates from `CrossPlatform/`, using four jobs, nice 10 and a
12 GiB user scope. Keep source, targets and generated files outside `/tmp`.
Build the test binary so the ignored native context probe is available.

Copy the evidence scripts into `CrossPlatform/target/niri-geometry-sol/`.
Its `start.py` expects the minimal adapter at `niri/target/debug/niri`. Generate
`input/pointer.h` and `input/pointer-code.c` with wayland-scanner from the pinned
wayland-protocols-wlr 0.3.12 virtual-pointer XML, then compile `input/pointer.c`
with `-lwayland-client` to `pointer` beside the scripts. The pointer helper
refuses any host runtime or the outer lab display. Python GTK/dbus and the local
Chrome executable are required. Browser profiles/temp sockets stay under a short
private cache path; no logged-in profile is used.

From the repository root:

```sh
agent-lab start koett-niri-geometry-sol
systemd-run --user --scope -p MemoryMax=12G nice -n 10 \
  agent-lab run koett-niri-geometry-sol -- python3 \
  "$PWD/CrossPlatform/target/niri-geometry-sol/start.py" "$PWD"
# Repeat with env NIRI_GEOMETRY_STOCK=1 before python3 for unmodified niri.
agent-lab stop koett-niri-geometry-sol --force
```

Sources: [pinned niri hit-test/pick source](https://github.com/niri-wm/niri/tree/8ed0da44d974c32c6877d2f4630c314da0717ecb/src),
[the unmerged upstream viewport proposal](https://github.com/niri-wm/niri/pull/4147),
and [AT-SPI coordinate API](https://docs.gtk.org/atspi2/method.Component.get_extents.html).
The source patch and its build restrictions are documented in
`CrossPlatform/integrations/niri/README.md`.
