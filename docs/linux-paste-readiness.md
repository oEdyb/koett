# Linux automatic paste readiness

## Findings

A native GTK target in a private nested niri 26.04 (`8ed0da4`) received
Ctrl+Escape from the previous `wtype` paste command even though `wtype` exited
successfully. Its event was keysym 65307, GDK keycode 9, Control mask 4; no text
was inserted. `wtype` assigns V a slot in its temporary keymap. If the compositor
uses/restores the native map, that slot denotes Escape. A successful process exit
therefore did not prove that a paste chord reached the target.

The niri fallback now reads the compositor's keymap metadata on a surface-less
Wayland connection and resolves V in the active layout. It sends the same map,
the correct native key and the configured Control/Control+Shift modifiers through
the virtual-keyboard protocol. US/Swedish V is evdev 47, rather than Escape's slot;
Dvorak is resolved separately. Setup and synchronization have deadlines. Setup
failure does not fall back to the faulty command or retry a possibly posted key.
Other compositors retain their existing portal/wtype delivery path.

There was a second, independent refusal: niri reports no focused window while a
Koett exclusive drawing layer owns input. Capture may use the active workspace's
window only when a tracked Koett keyboard layer is actually active. Delivery
waits up to one second for that layer/temporary missing focus and physical
modifiers to release. The original window must return and remain stable before
posting. An unrelated focused window, a re-mapped window ID or a missing original
target is refused. A final focus/modifier check runs after native keyboard setup.
Physical keyboard access being unavailable does not become a new permission gate.

The successful key post is never retried automatically. Genuine focus changes do
not authorize typing into a different window: it may be another conversation or
a terminal. The pill stays pending until delivery finishes. Failure keeps
**Not pasted · saved**, **Paste** and **Copy**. Explicit Paste chooses the current
external window and binds to it for that operation. A failed recovery Paste also
restores the recovery pill, rather than only displaying a transient notice.
The pill's dimensions and controls are unchanged.

## Historical host evidence

The six-hour journal sample contained 68 delivery steps. Three failed dictations
ended before any `paste_post` event:

- 2026-11-04 20:40:18 UTC: PID 1236685, step 15, text, 73 bytes.
- 2026-11-04 21:57:29 UTC: PID 1586927, step 3, text, 26 bytes.
- 2026-11-04 22:09:26 UTC: PID 1721867, steps 2/3, image 654423 bytes,
  text 37 bytes.

These older records do not contain refusal reasons or window IDs, so their exact
individual causes cannot be reconstructed. They do not prove the keymap fault:
that fault was reproduced separately after a key was posted. The newer installed
app's stderr was `/dev/null`, so absence of later journal steps is not evidence
that delivery never ran. No History text was printed or copied into this receipt.

## Future diagnosis

Metadata goes to stderr and private rotating files, independent of the launcher:

```
~/.local/state/koett/delivery.log
~/.local/state/koett/delivery.previous.log
```

Each file rotates around 256 KiB; new files use 0600, the directory 0700. Entries
include PID, timestamp, step ID/kind, attempt 1, byte count, expected/observed
window IDs, physical modifier mask when available, virtual native key/mask,
backend and outcome/reason. No text, image bytes, window titles, ordinary keyboard
events or clipboard contents are logged. Query and key posting failures are
separate from `focus_changed`, `focus_unavailable` and `modifiers_held` refusals.
`paste_post` means the injection boundary was reached, not target consumption.
Wayland provides no universal application-side paste acknowledgement.

## Verification

Private lab `koett-paste-focus`, with copied ASR models and a private audio source;
no host typing, focus changes or clipboard changes. Committed machine-readable
receipts are in `CrossPlatform/evidence/linux-paste-focus/`.

- Production delivery into native GTK: exact text once, one actual Ctrl+V.
- Exclusive layer: niri really returned no focused window; capture retained the
  underlying target. Releasing the layer allowed one insertion. Keeping it held
  timed out with no key or text delivered.
- A different window was refused without insertion; explicit recovery inserted
  once into the chosen current window.
- Full Koett app, copied-model audio through the fallback backend: the 121-byte
  final result matched the target buffer exactly, with one paste. A second
  recording with an actual focus change saved once and inserted nothing. The
  actual accessible pill showed Not pasted/Copy/Paste; activating Paste inserted
  that saved result once into the current window. Transcript contents were
  compared in memory and not logged. Drawing opt-in was tested separately in the
  exclusive-layer fixture; the focus-change app journey had drawing disabled so
  another real target could take focus during recording.
- Chrome rich editor and kitty received exact text once through production
  delivery, with Ctrl+V and configured Ctrl+Shift+V respectively.
- The extra private T3 chat-box check was blocked at its pairing/bootstrap screen;
  it is not counted as a successful target check. No message was sent.
- Unit regressions cover US/Swedish/Dvorak key resolution, malformed maps,
  synchronization timeout, focus restoration, held modifiers, final post checks,
  refused changed/remapped targets, explicit recovery binding, query timeout,
  bounded private logging and pending/recovery result messages.
- After rebasing onto the draw-lock merge, full Rust suite: 420 passed,
  19 ignored. Clippy with warnings denied, formatting and release build passed.
  All 50 supply-chain script tests,
  inventory/SBOM validation and 700 notice files passed; policy unchanged.
- Local Windows cross-check is unavailable: the ordinary check rejects the
  Linux ASR library receipt for a Windows target; the metadata-only attempt also
  needs the unavailable MSVC `lib.exe`. Windows CI remains required.

The Mac uses physical macOS V keycode 9, so this Wayland keymap fault does not
apply. The source-level held-modifier/temporary-focus refusal had an equivalent
on Mac; main now contains the separate PR #81 readiness fix. See
[Mac paste readiness](mac-paste-readiness.md). No Swift changes are part of this
Linux patch, and no Mac runtime test was performed here.
