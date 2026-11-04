# Optional niri pin geometry adapter

Stock niri 26.04 does not expose tiled window origins. This optional source
patch adds `koett_geometry_v1` only to `PickWindow` replies. Koett consumes it
when present and retains its existing stock-niri fallback when absent. It does
not infer positions from column order, gaps, focused windows or animation targets.

The snapshot comes from the compositor's input hit test before the pick releases
its grab. It contains the global pointer, buffer-relative pointer and visible
window bounds in logical coordinates. Only input-region hits carry geometry;
borders, overview activation hits and unavailable windows keep it absent. Normal
window lists/event streams do not carry geometry or acquire per-frame updates.
The existing pick/focus/shield behavior is retained.

This is a Koett-specific extension, not an upstream niri release or an automatic
system integration. The source pin is niri v26.04,
`8ed0da44d974c32c6877d2f4630c314da0717ecb`. The patch derives from that project's
GPL-3.0-or-later source; retain upstream licensing when distributing the patched
compositor. Koett's Rust executable does not link or vendor the compositor.

## Build without installing

Check available memory first. Clone the pinned source outside `/tmp`, then run:

```sh
CrossPlatform/integrations/niri/build-adapter.sh /path/to/clean/niri
```

The script checks the source commit and clean state, applies the patch and builds
with four jobs, nice priority 10 and a 12 GiB user scope. It writes to that
checkout's `target/`. It deliberately builds the minimal nested-test compositor
without default D-Bus/systemd/screencast features. A desktop replacement needs
those normal distribution features and its own packaging/session acceptance.
The script never installs, starts a compositor, changes a session, edits desktop
configuration or touches the running host compositor. Do not switch an active
user session to a private test binary.

A plain Koett update cannot make an unpatched niri expose missing coordinates.
The stock-niri parity row remains partial until a compatible compositor API is
available or an explicit adapter is deployed and accepted. Missing or rejected
geometry keeps image/title/page/selection fallbacks; no guessed link/image hit.
