# Linux pin context: row 3 progress

2026-11-04. This PR adds YouTube time, native KWin window geometry, and a
provider-based fallback for other desktops. **It does not close the stock niri
tiled-window pointer-hit gap or GNOME drawing-overlay gap.** No default/model or
Windows/shared engine change. Main with #54/#57 was separately installed at
`e1ba8dd61c09fffb7f34942e99bd59fe65bb41af`; this PR was not installed.

## Behavior

- YouTube: only `ytp-progress-bar` on the matched visible web document supplies
  seconds through AT-SPI Value.CurrentValue. Volume and other sliders are ignored.
  Existing `t`/`start` parameters are replaced; video identity and other parameters
  stay. Missing/invalid time leaves the original URL. No MPRIS association guess.
- KWin: a one-shot read-only script gets the top window at the pin and its buffer
  geometry (including surface margins used by window-relative accessibility coordinates). It neither clicks nor changes focus. Own overlays are excluded.
  A dedicated temporary D-Bus reply accepts only the KWin service owner. The script
  is unloaded and its private scratch file deleted after the bounded query.
- Other desktops: AT-SPI showing-frame geometry can identify a unique frame;
  unavailable/zero-origin geometry, overlaps and ambiguous matches give no guessed
  context. Monitor offsets, including negative origins, are passed explicitly.
- Pointer hits descend into the accessible child, validate ancestry back to the
  matched frame, and inspect the corresponding document. A linked image may supply
  both its destination and image URI/label. Password nodes are not read.
  Selected-text transfer is capped at 500 characters before requesting text.
- Existing niri pick/focus/grab safety is preserved. When geometry is unavailable,
  niri still returns app/title and available page/selection/YouTube time; the image
  and pin bullet remain usable without fabricated pointer metadata.

## Stock niri limit, still open

Installed niri: `26.04 (8ed0da4)`. Its scrolling layout populates
`pos_in_scrolling_layout` but not `tile_pos_in_workspace_view`. This is also true
in the upstream source inspected for this PR. Native Wayland Chrome in the
private KWin check reported identical screen/window origins `(0,0)` despite a
nonzero compositor window position. Treating that as global geometry would attach
another element's link to a pin. This PR rejects that guess.

A niri geometry API/integration is still required for reliable tiled native
Wayland browser pointer hits; no compositor patch, inferred gap arithmetic,
clipboard-changing screenshot command or privileged helper was installed.
This is a specific current integration limit, **not an OS-wide impossibility**.
GNOME currently lacks the drawing overlay backend as a separate Linux gap.

## Fresh checks

- `cargo fmt --check`: passed.
- `cargo test --locked`: 375 passed, 17 ignored in the library suite, plus native
  ABI test passed. The three added ignored tests require private desktop fixtures.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo build --release --locked`: passed.
- `cargo check --locked --target x86_64-pc-windows-msvc`: blocked while building
  bundled SQLite because MSVC `lib.exe` is absent on Linux.
- `git diff --check`: passed.

Builds used `CARGO_BUILD_JOBS=4`, `nice -n 10` and user scopes with `MemoryMax=12G`.
Build output stayed in the worktree. No subagents or host pointer/keyboard input.

Private KWin desktop `koett-row3-sol`, private Chrome profile, native Wayland:

- A real YouTube page exposed `Seek slider`, class `ytp-progress-bar`, current
  value `0.0`, and a separate `Volume` slider with value `100.0`.
- The Rust pin reader returned
  `https://www.youtube.com/watch?v=jNQXAC9IVRw&t=0s`, with the matching page title.
  This proves the actual page/slider path; it does not prove seeking, every locale,
  advertisements or fullscreen playback. Pure fixtures cover nonzero seconds.
- The actual linked-image fixture returned link `https://example.com/pin-target`
  and image label `Coral square`. Its file URL was intentionally absent from the
  resulting page URL. The authenticated KWin callback was enabled for this check.
- The independent KWin geometry test returned the browser's nonzero window rect.

These tests exercise the context reader against live providers, not a complete
Shift-click/capture/paste session. Existing pin gesture, capture and delivery
paths were not rewritten. Real niri multimonitor, GNOME and generic AT-SPI provider
acceptance remain unverified. No claim that all row-3 platforms are complete.

## Primary references

- [niri IPC layout fields](https://github.com/YaLTeR/niri/blob/main/niri-ipc/src/lib.rs)
  and [scrolling IPC layout population](https://github.com/YaLTeR/niri/blob/main/src/layout/scrolling.rs).
- [AT-SPI Component coordinates and hit tests](https://github.com/GNOME/at-spi2-core/blob/main/xml/Component.xml)
  and [Value scalar properties](https://github.com/GNOME/at-spi2-core/blob/main/xml/Value.xml).
- [KWin windowAt and bufferGeometry](https://develop.kde.org/docs/plasma/kwin/api/).
