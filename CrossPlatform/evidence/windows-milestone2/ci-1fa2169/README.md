# Rebuilt M2 runtime checks

Unmodified head 1fa2169437a3c5f05e4b36a18f968d6bc4046324; CI run 41928424781,
artifact 13371951025, tested merge e89283ab90ebe6e5d6170408d6cd3fc9d656d887.
ZIP SHA256 52aa8715bc012ba2b8a4f2dfbc6a715f991ee43b8245d9e9885d9c03415c297f.
EXE SHA256 a748f48808a25673d6c61a9f4925c6fb0e5d0138886cd8d8c40fc92f4e823876.

Windows 11 KVM, 4 vCPU/8 GiB, all observed guest threads nice 19. Initial host
39 GiB MemAvailable, 75% idle; later sample 81% idle. No own host builds while
testing. Guest fully shut down; both local transfer servers stopped afterward.
Only guest input/data used. These are real CPU/session/delivery tests using
--dictation-lab-wav, not synthetic --desktop-lab text. Fixture SHA256
5fceacff0315d49cb59fcc505bcecf1ed5f2f35c2897b1e65a59f30e5d922150.

## Passed on the linked executable

- Notepad: empty editor received exactly the recognized clipboard text once,
  unchanged foreground HWND/PID; one corresponding history row.
- Hold: real Ctrl+Shift+Space down/up started/stopped once, exact text delivery.
- Pause/resume: accessible states changed; resumed result omitted the paused
  section and matched the clipboard. The changed recognized wording is retained
  in JSON, not represented as a speech-accuracy pass.
- Cancel: editor remained empty, pill hidden, no additional history row.
- Edge textarea: exact recognized text, one DOM paste event, stable foreground.
- Windows Terminal: exact text read by Console.ReadLine into a receipt file;
  transcript was data and never executed as a shell command.
- Foreground switch during ASR: both editors untouched, persistent Not pasted
  with Paste and Copy. Paste refused the other target, then succeeded once after
  the tester returned to the original editor. Retry did not create a history row.
- Read-only history DB: paste still succeeded, database hash unchanged, exactly
  one recovery Markdown file preserved the transcript; attribute restored.

History snapshots were copied from the private guest to a private cache and
queried locally: 3 recognized rows after Notepad/hold/pause/cancel, 5 after the
browser and terminal. Database snapshots are not committed. Every saved step
has a screenshot and foreground HWND/PID. The browser's first attempt stopped
before recording because the page was not ready; the final check used the loaded
textarea and verified its Edit role.

## Failed on this artifact; fixed in source, not yet retested

1. Elevated editor: administrator token level 12288, Koett level 8192. Target
   remained empty but pill showed Done. This demonstrates that input event count
   cannot establish delivery. Source now queries process token integrity before
   input, refuses higher-integrity or uninspectable targets, and shows recovery.
   No privileged helper or app elevation was added. The isolated elevated
   WinForms editor used UAC consent on the guest only, then was closed.
2. Combined changed-focus and read-only-history failure: fallback preserved the
   text, but the history warning replaced Not pasted and hid Paste/Copy. Source
   now prioritizes delivery recovery for every storage outcome and distinguishes
   saved from unsaved in drawing and UIA. A late queued transcript is also ignored
   if its session was cancelled before UI delivery.

Eight portable pill/state tests pass, including the delivery/storage outcome
matrix. Windows metadata Clippy compiles integrity token and UIA action tests;
Windows CI executes those. Native Linux tests and strict checks are recorded in
the PR. New source still needs its rebuilt-executable elevated/combined retest.

M2 is not complete. Remaining: these regressions, hotkey conflicts, silent/short
and missing-input desktop cases, ASR failure/audio retention, busy/full storage
and fallback exhaustion. Physical USB/Bluetooth/privacy/sleep and 30-minute live
capture need hardware. The earlier CPU CLI silence test remains separate from
a complete desktop silence gate. No generic target acknowledgment is claimed:
Windows provides no atomic paste receipt for arbitrary apps.
