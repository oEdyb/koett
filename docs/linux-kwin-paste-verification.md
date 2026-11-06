# KWin paste target identity

> Historical evidence and commands use the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). See [the evidence archive](evidence-archive.md) for private reproduction.

Linux now checks the original window on KWin as well as niri/X11. It reads
KWin's active window, UUID, PID and exact resourceClass through the existing
sender-authenticated, read-only scripting adapter. It never selects, raises or
activates a target. Window titles, page contents and screenshots are not read
for this check. The pin-at-pointer query reuses the same transport; its body and
geometry fields remain unchanged.

KDE documents [workspace.activeWindow](https://develop.kde.org/docs/plasma/kwin/api/)
and [Window.internalId](https://api.kde.org/qml-org-kde-kwin-window.html#internalId-prop)
as the active-window property and unique window UUID. PID alone cannot distinguish
two windows in the same app; the guard uses the UUID and KWin service incarnation.

## Behavior and defaults

- Each capture/delivery check asks the compositor anew. No last-known focused
  window is reused after timeout, failure or an unavailable service.
- A single worker handles synchronous callers without nesting Tokio runtimes.
  One queued request and a 200 ms caller wait budget bound backpressure. Expired
  queued requests are skipped and late results are dropped. Backend work can
  finish cleanup after the caller has already fallen back to manual paste.
- UUIDs receive process-local, never-reused u32 tokens for the existing shared
  engine interface. The map holds up to 4096 distinct window/service pairs;
  overflow uses manual paste until restart. Only identifiers are cached, not focus.
- Own-process/Koett windows, missing/invalid identifiers and desktop/dock windows
  are rejected. KWin restart gives a new identity namespace.
- Exact KWin resourceClass strings select the existing per-app image rules.
  `koett paste-target-status` reports availability, backend and `app_id`; use that
  value as the rule key. This command reports the Wayland identity adapter;
  X11 retains its separate window-ID guard. The key may differ from an app's
  niri ID. Text retains its
  separate configured chord. No lowercase/substring matching or double chord.
- A concrete image target must still be available when resolving the chord.
  Explicit pill Paste retains its existing immediate-user-action fallback.
- niri retains its own IPC path. Other Wayland desktops without an adapter keep
  copy/manual paste. GNOME identity remains part of the deferred Shell work;
  other compositor adapters are still named backlog, not OS-impossible.

This adds identity, not a keyboard/clipboard transport. KWin automatic delivery
still needs a working authorized portal or compatible backend. Unknown identity,
denied permission and unsupported injection keep text copyable. The focus check
and key posting are not atomic and do not prove arbitrary target consumption.
No new settings, model selection, history schema or persistent KWin script is added.

## Verification, 2026-11-04

- 389 Rust tests passed, 17 ignored, plus native ABI subprocess check.
  New coverage checks stable/distinct window tokens, compositor restart, invalid
  and own identities, exhaustion, exact app rules and lost identity at chord
  resolution. Obsolete tests for the removed niri-only boolean helper were removed.
- Rebased on main `778c901` with the captured-drawing undo changes retained.
  The private app returned drawing status and the expected exit-1 `no_drawing`
  response for empty Undo, alongside working paste-target status.
- Formatting, clippy with warnings denied, release app/example builds and diff
  check passed. MSVC cross-check is blocked here by missing Microsoft's `lib.exe`.
- Real KWin 6.8.2-1.1 private desktop, copied 110M model, release app/control socket
  and two native GTK windows in one process. Paste Last reached window A with exact
  Unicode text. Switching to B during the clipboard write produced zero further
  key posts and left B untouched; copied text remained available.
- Exact real app-class image-rule resolution selected Ctrl+Shift+V while the text
  preference stayed Ctrl+V. No active target and Koett's own window were rejected.
- Twenty identity-status CLI round trips: median 21.2 ms, max 22.4 ms.
  This includes client startup; it is not a universal latency guarantee. A 700 ms
  private-compositor stall returned unavailable in 217.9 ms, then a fresh
  check recovered. No last-known focus was returned during the stall.
- Production read-only scripts were unloaded and scratch directories were absent
  after checks. The existing isolated niri fixture also passed unchanged-target,
  unknown/changed-target, exact clipboard and configured-chord cases. Its niri
  responses/key sender are fixtures, not a new real-niri acceptance claim.

The KWin key sender was a test-only `wtype` shim mapping to the private KWin input
socket. This proves one real GTK text consumer and the production guard path,
not native wtype support or portal permission grants. No host input/clipboard or
installation of this PR was performed. The inspected screenshot includes the expected
Paste failed notice from deliberately changing the target, with the text in A.

### Image transport remains open

The full fallback PNG exercise failed because this KWin lab does not advertise
the wlr data-control clipboard protocol used by Koett's custom owner. The error
was `Compositor lacks data-control clipboard support`. Do not count this as
image consumption or complete KWin image-paste acceptance. The portal clipboard
path and grants, repeat PNG/HTML/text consumption, and any additional fallback
backend remain open. Working text paste is not evidence that every image backend
works. Pin-at-pointer geometry was not independently re-run in this change.

Evidence and fixtures: `CrossPlatform/evidence/kwin-paste-identity-20261104/`.

## Reproduce

Check available memory and build in CrossPlatform under CARGO_BUILD_JOBS=4,
nice -n 10 and a user scope with MemoryMax=12G. Also build the release
`fallback_clipboard_probe` example. Copy a verified 110M model into
`CrossPlatform/target/kwin-focus-sol/model` (no symlinks). From the root:

```sh
mkdir -p CrossPlatform/target/kwin-focus-sol
cp CrossPlatform/evidence/kwin-paste-identity-20261104/{check,target}.py \
  CrossPlatform/target/kwin-focus-sol/
agent-lab start koett-kwin-focus-sol
systemd-run --user --scope -p MemoryMax=12G nice -n 10 \
  agent-lab run koett-kwin-focus-sol -- python3 \
  "$PWD/CrossPlatform/target/kwin-focus-sol/check.py" "$PWD"
agent-lab stop koett-kwin-focus-sol
```

Fixtures refuse other runtimes. The compositor stall and input shims operate
only inside that private desktop. The app exits through guarded Quit. No virtual
Shift is generated; the keyboard-test environment is pinned to an unused lab name.
