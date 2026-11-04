# Linux Paste Last control command

`koett paste-last` now runs the same controller delivery path as the native
Paste Last action. It restores the persisted last transcript after restart and
uses the current configured text chord. If the session still has a matching
sketch, Wayland delivery also retains the existing image/text/bundle path.
It does not save another history entry.

The control listener captures the focused niri/KWin window identity or X11 input window
before queuing. The controller checks that same identity at delivery; a CLI
request never uses the pill's immediate-click focus bypass. Requests that wait
three seconds before starting expire without touching the clipboard or keys.
The listener waits up to nine seconds for completion. A missing response is an
uncertain outcome: inspect the target before retrying. Unknown/changed focus
copies and reports an error; unsupported Wayland desktops retain manual paste.
No target is activated. Successful JSON says `paste_posted` and
`receipt: key_posted_only`: key posting is not proof of application consumption.

## Verification, 2026-11-04

- `cargo test --locked`: 379 passed, 17 ignored, plus native ABI subprocess check.
  Includes request expiry/result fixtures and existing delivery focus guards.
- `cargo fmt --check`, clippy with warnings denied, release build and diff check
  passed. Linux-to-MSVC check cannot complete without Microsoft's `lib.exe`.
- Private KWin lab `koett-row6-sol`, copied 110M model, private config/history and
  real release app/control socket/Wayland clipboard. Fixed fake niri responses
  and a key-sender recorder isolate focus races without host input.
- Restored Unicode text matched the clipboard byte-for-byte. Unchanged target
  posted exactly the configured Ctrl+Shift+V chord, exit 0; unknown and changed
  targets posted no keys, exit 1 with manual-paste text. Durable Last remained
  unchanged and no transcript history row was inserted.

This is not a real niri/X11 target-consumption test, portal grant check or fresh
image-paste acceptance. Those existing desktop/clipboard matrix gates remain
open. Test fixture and payload-free results are under
`CrossPlatform/evidence/last-paste-20261104/`.

Reproduce after the release build (check available memory first):

```sh
mkdir -p CrossPlatform/target/row6-sol
# Copy a verified local 110M model into target/row6-sol/model; never symlink it.
agent-lab start koett-row6-sol
systemd-run --user --scope -p MemoryMax=12G nice -n 10 \
  agent-lab run koett-row6-sol -- python3 \
  "$PWD/CrossPlatform/evidence/last-paste-20261104/check.py" "$PWD"
agent-lab stop koett-row6-sol
```

The fixture refuses to run outside this named private lab. It writes only under
`CrossPlatform/target/row6-sol`, including its copied model/settings/history.
It does not press Shift or send keys to a desktop; its key sender is a recorder.
The private app reaches idle before the tests and exits through guarded Quit.

KWin identity and its private native-consumer checks were added separately; see
[KWin verification and transport limits](linux-kwin-paste-verification.md).
The original niri fixture above remains a fixture, not a full desktop matrix.
