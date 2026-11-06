# Recording-scoped drawing key tap

The always-on consuming modifier/mouse tap previously included keyDown and keyUp.
Its callback ran on the main run loop, making ordinary typing wait when that
thread stalled. The main tap now contains only modifier and mouse events.
Drawing owns a separate key-only tap, created after drawing starts in a recording,
and invalidated on Stop before file close, Cancel, or leaving recording. A failed
installation disables drawing for that session with a visible notice; dictation
continues. Passive custom-shortcut monitoring remains and cannot consume input.

23 impacted Swift tests and a release build pass; the final local-control
assertion also passes its six-test group. Test controllers without installed
hotkeys never create global event taps. No full suite or benchmark cohort ran.

Guest runtime source `09bfbe2e8fec940e35ed54239532c7271fc641c5`, macOS 15 disposable
profile. Exact source/hash bindings, guest driver and CGGetEventTapList collector:
[evidence](evidence/macos-recording-key-tap-20261106/tap-receipt.json).

- Before fix, the idle process owned an enabled consuming tap whose mask included keys.
- After fix, idle has no consuming key tap. A synthetic A reaches an owned native
  recipient while Koett is SIGSTOPped, before it is resumed in a finally block.
- Drawing-enabled recording creates exactly one consuming key-only tap. The draw
  shortcut and Cmd-Z work without sending D/Z to the recipient; undo removes one stroke.
- Stop returns transcribing with no consuming key tap remaining.
- A new launch with drawing off records without a consuming key tap; Cancel leaves none.
- The guest drawing setting is restored to on; the executable matches the staging hash.

These are functional routing checks, not throughput measurements. No hardware
microphone, physical keys, real-host input, installed app or frozen ZIP was used
or changed. The guest native recipient source is retained with the preceding
[drawing undo evidence](evidence/macos-draw-undo-consumption-20261106/KeyReceiver.swift).
