# Active-display default for the Mac pill

An undragged pill now receives `NSScreen.main` as its preferred display, restoring
the original active-display default. A valid remembered drag always overrides it.
A missing/corrupt saved display falls back to that active screen without erasing
the preference; if the preferred screen itself is unavailable, the first available
screen remains the safe fallback. Pill size, appearance and button routing do not change.

Nine PillPositionTests pass, including an active second screen despite primary
being first, a saved drag overriding the active screen, and a third-screen fallback
that preserves the disconnected display's saved position. Release build passes.

Guest runtime `f3d4dd5341b9d016de51a4e94d61df9b569b6fa9`, macOS 15 disposable admin:

- Without a saved drag, the real 300×48 panel is at the active screen's bottom-center
  position (362,595 in Quartz coordinates), independently computed from NSScreen metadata.
- An actual background drag moves it to (150,200), still 300×48.
- Recording after a restart restores that drag.
- A simulated unavailable saved display falls back to the active screen's default,
  while its stored display ID remains intact.
- Guest preferences are restored; source/executable hashes match staging.

[Exact driver, screen probe and receipts](evidence/macos-pill-active-display-20261106/pill-receipt.json).
The guest has one display. Multi-display preference, negative coordinates and
reconnection are covered by geometry tests, not physical multi-monitor hardware.
No host screen, microphone, input, installed app or frozen ZIP was used or changed.

One staging attempt began before the release build completed and was discarded
without launching it. Acceptance uses a fresh archive assembled only after the
successful build exit. It is not evidence for a new frozen or published release.
