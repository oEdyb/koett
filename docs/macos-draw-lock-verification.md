# Mac draw lock

Implemented in PR #130. Executed source:
`6f0086fd9f02e24420a2b0757ebdab82238f635b`.

- Default Control-Option-D, recording-scoped; the menu shows a conflict-free
  alternate chord if a dictation/media binding already uses the default.
- The event tap consumes only that chord's down/repeat/up events while recording.
  An initial observer-only prototype inserted a control character into NSTextView;
  the corrected real guest run asserted the target remained empty before delivery.
- Left-button strokes join one sketch per display; hovering adds no ink.
  Shortcut or first Esc captures/unlocks; another distinct Esc cancels dictation.
  Esc repeats cannot turn unlock into cancel. Right-click undoes one stroke;
  observed Command-Z/Control-Z also reach the focused app, as on Linux.
- Stop captures open lock ink. Cancel discards it and late captures.
  Pill controls remain above the lock surfaces; appearance and size are unchanged.

## Checks

95 impacted Swift tests initially passed (one optional skip); after repairing
chord consumption, 98 impacted tests passed (one optional skip), including the
shortcut/modifier callers. Release product build passed in 47.92 seconds.
No full-suite repeat or benchmark was run.

A signed app in a private macOS 15.7.7 guest passed eight checks: idle shortcut
is inert; active chord is consumed; two strokes produce one captured/pasted
sketch; first Esc preserves recording; that Esc's capture pastes on Stop;
second Esc cancels without a History write; Stop captures while locked; real
pill Pause remains clickable at 300×48. Each completed drawing added exactly
one History row and real Command-V reached an owned native NSTextView. The PNG
was inspected: two separate red strokes in a single image numbered 1.
Executable SHA256: `ecbe126011d15561e863248c0cbee6fad50f010ca4f3568f8220e40bd2b49334`.

Physical keys, microphone, multiple physical displays/mixed DPI, tablet input,
and other target editors are untested for this change. The guest used a silent
BlackHole input and synthetic CGEvents. Real right-click/keyboard undo has
unit/model coverage, not a separate guest pointer/key receipt. No host input,
installed-app replacement, or frozen v0.3.0 ZIP change was performed.

## VoiceOver follow-up

PR #133 preserves [VoiceOver's Dock command](https://support.apple.com/guide/voiceover/vo2695/mac)
by using [NSWorkspace.isVoiceOverEnabled](https://developer.apple.com/documentation/appkit/nsworkspace/isvoiceoverenabled).
When VoiceOver is running, the default draw-lock chord becomes Control-Shift-D;
Control-Option-D is never chosen, even if other bindings occupy the alternatives.
Menu/CLI remain available when no chord is free.

31 impacted Swift tests passed, zero failures; release product build passed in
47.67 seconds. A signed build from `e135197c2646ded671d7deec9c57be3e848e4378`
passed actual macOS 15.7.7 guest VoiceOver on/off checks: while on, Control-Option-D
did not toggle draw lock and Control-Shift-D toggled on/off; after restoring
VoiceOver off through Settings, Control-Option-D toggled on/off again.
Executable SHA256: `25758354852488336c53c167f128599844ebb8ab754e4085962cd08e7af8e354`.
These were synthetic CGEvents, not physical-key or assistive-user navigation
acceptance. Guest audio sharing was disabled. VoiceOver was restored off.
