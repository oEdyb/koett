# Mac drawing undo verification

Result: **PASS for the existing recording-scoped menu and CLI actions** on
macOS 15.7.7 / 24G720, 2026-11-05. No new undo implementation was needed.
The earlier Scrawl comparison incorrectly described this as missing.

## Actual guest flow

Private signed source builds `f140724` and `8867873`; generated audio and an owned
Chrome editor in the `koett-macos15-audit` Tart guest. Natural Microphone,
Accessibility and Screen Recording approval; real capture-bypass consent allowed.
No host input, clipboard, installed app or real data changed.

1. Enable **Draw & Pin While Dictating** through the real menu.
2. Start actual recording through `koett --control start`.
3. Inject a complete Shift-drag using the guest-only CGEvent helper. Ink and a
   numbered drawing band appear in the real overlay.
4. Invoke **Draw on Screen → Undo Last Drawing** through Accessibility. The action
   succeeds; the next `koett --control undo` returns `no_drawing`.
5. Repeat with an actual Shift-click pin. `koett --control undo` succeeds.
6. A second undo after removing the last mark returns `no_drawing`, preserving the
   recording. Cancel the test session.

The same source contains `SketchOverlay.undoLast`, which removes the frame and
publishes updated numbering. Pending capture completion checks the group ID;
removed groups cannot return later. `RecordingParityTests` cover captured/pending
undo and session isolation. The full suite for the subsequent paste fix passes
439 tests, 15 skipped, zero failures; no new tests were added for this doc change.

## Limits

Start/stop used the public local-control interface. Guest CGEvent gestures are
actual overlay input, but do not certify physical keyboard hardware. No global
Cmd+Z interception was added, and editor undo remains owned by the target app.
Multi-display and physical input verification remain release-audit follow-ups.

Sources: [overlay](../Sources/Koett/SketchOverlay.swift),
[menu](../Sources/Koett/SketchDictation.swift),
[control action](../Sources/Koett/ControlCommands.swift),
[guest audit receipt](evidence/release-v0.3.0-mac/guest-paste.json).
