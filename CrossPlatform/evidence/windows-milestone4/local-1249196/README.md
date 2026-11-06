# Local Windows M4 verification — 1249196

Application SHA-256: `124919625d4acce1b0fc8f818011984b9c46680cb3b2d90a1fdea38a6d14e874`.
Private locally linked development build, not a release.

- All 17 impacted native Windows tests exited 0. Clipboard tests exercise actual
  delayed text rendering, guarded combined-offer restoration and newer-copy
  protection. This does not by itself prove delivery in an external editor.
- The guest virtual HDA microphone preview started with one worker and stopped
  with none remaining. Physical microphone signal quality was not tested.
- Forced enumeration stalled, displayed a timeout and cleaned up its worker.
  Two preview rounds each timed out in about 5.5 seconds, kept pending controls
  disabled and left no worker. Three extra queued commands created no extra child.
- Default and named microphone hotkeys each retried twice. All four attempts
  timed out visibly in 5.0–5.2 seconds, retained Edge HWND 196788 / PID 6876 and
  left no worker. These are injected worker stalls, not real driver faults.
- Native menu probing reproduced blank UIA names for shortcut-only labels.
  Adding an explicit tab separator preserved both exact accessible names.
  The next build contains that fix; this receipt does not claim its full menu
  journey passed.

The settings, history and last-result hashes were restored before guest shutdown.
No host input or personal Koett profile was accessed. Chrome is not tested.
