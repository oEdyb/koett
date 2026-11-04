# Linux recording-scoped drawing undo

The native menu and tray now expose **Undo Last Drawing**, enabled only while
a dictation has drawings. `koett undo-drawing` uses the same overlay operation;
`koett drawing-status` reports activity, total/captured counts and availability.
Neither command exposes pixels, text or pin context.

Undo removes the last whole drawing or pin in creation order, its stored PNG
and its pill band. It works during drawing, capture, context reading and after
capture. Capture workers may still finish, but removed IDs cannot reach delivery.
Orders remain monotonic within a recording; displayed numbering stays contiguous.
The next recording clears both frames and removed IDs. Undo after stop/cancel or
when empty returns `no_drawing`, exit 1. Requests queued more than two seconds
expire without action; after a missing response inspect status before retrying.

No global Ctrl+Z registration is added. Existing focus-mode Ctrl+Z still undoes
strokes in uncaptured ink. Normal editor Ctrl+Z is unaffected by this new action.
This does not add drawing support to GNOME or X11.

## Evidence, 2026-11-04

- `cargo test --locked`: 382 passed, 17 ignored, plus native ABI subprocess check.
  New frame-store tests remove completed and pending frames, reject late results,
  retain unrelated frames and reject stale-recording removals.
- Formatting, clippy with warnings denied, release build and diff check passed.
  The MSVC cross-check is blocked here by missing Microsoft's `lib.exe`.
- Real release app, CPAL silent microphone capture, GTK overlay, private KWin
  desktop `koett-undo-sol`, copied 110M model and isolated config/data/cache.
- A grabbed, explicitly named test keyboard supplied Shift only inside that lab
  with `KOETT_TEST_DRAW_KEYBOARD`; pointer movement used the private KWin socket.
  No host focus, input or clipboard was changed.
- A deliberately delayed PNG fixture exercised undo during capture. Status went
  from one pending group to zero and remained zero after the late result.
- Two more draws reached `captured_count: 2`. Activating the native menu's GTK
  action through its session-bus action interface removed one; CLI undo removed
  the other. Extra/idle undo failed. Cancel and the next recording started empty.
- Inspected native screenshots show two numbered pill bands before undo and one
  after, with the removed drawing no longer visible.

Committed fixture, result JSON and screenshots:
`CrossPlatform/evidence/drawing-undo-20261104/`.

The screenshot backend returned a known full-size PNG; actual screen permissions,
portal screenshots and image consumption by a target were not exercised here.
This checks drawing/capture-result/undo behavior, not screenshot fidelity. The
tray entry is wired but was not clicked in a tray host. Multi-monitor/hotplug,
GNOME/X11 overlays and ordinary-editor runtime acceptance remain open.

## Reproduce

Check available memory first. Build from `CrossPlatform/` under the required
four-job, nice-10, 12-GiB scope. Copy a verified 110M model into
`CrossPlatform/target/undo-sol/model` (no symlinks), then from the repository root:

```sh
agent-lab start koett-undo-sol
systemd-run --user --scope -p MemoryMax=12G nice -n 10 \
  agent-lab run koett-undo-sol -- python3 \
  "$PWD/CrossPlatform/evidence/drawing-undo-20261104/check.py" "$PWD"
agent-lab stop koett-undo-sol
```

The fixture refuses other runtimes, uses isolated XDG directories, explicitly
releases the grabbed keyboard and closes the private app with Cancel/Quit.

Rebased onto main `e4feb54` after PR #66 merged: 384 tests passed, 17 ignored,
plus native ABI check; formatting, clippy and release build passed. The same
private-app workflow passed again, including native action and late-result undo.
The parity-document conflict retained both guarded Paste Last and drawing undo,
plus the open-row queue; the native Rust changes merged without a code conflict.
