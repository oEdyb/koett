# Final Windows M4 verification — 5b7b252

Final application SHA-256: `5b7b25207f002f496a0b6cf6af44e871ab5ac5ed52373a45201ca9156b83ec69`.
This is a private locally linked MSVC build, not a signed release.

This build adds only the final drawing-opacity and clipboard-validation order
changes to the source tested by local-9bb4c97 (source commit 5707ac9). The receipt
includes its Rust source manifest and exact patched native library hashes.

Passed on real Windows:

- Six focused native tests: Mac 450 ms pin / 1500+400 ms ink timing, static
  released marks under Reduced Motion, and all five final paste-guard cases.
  Clipboard validation changing foreground cannot redirect input; invalid
  clipboard data cannot send it. Every test group exited 0.
- All 15 shared Mac/Linux gesture fixtures, plus the seek-time parser's refusal
  to infer seconds from percentages, exited 0. Together with local-9bb4c97's
  20 tests, these are 42 distinct targeted Windows tests, not the full suite.
- Actual CPU WAV capture and physical Raw Input pinning with animations off.
  The screenshot shows the opaque pin; after release the canvases were hidden
  at the 600 ms check. The prior animation preference was restored.
- A physical pill Stop click while Shift was held, followed by release, pasted
  one image and one text event into the original Edge HWND 262366/PID 5860.
  The observed browser event gap was 366 ms; source timing uses read plus 200 ms.
  Target focus stayed unchanged. The combined clipboard restored all four
  formats automatically; its actual PNG is included.
- Two real Transcribing pill crops 400 ms apart were byte-identical with Reduce
  Motion enabled. The setting was restored afterwards.

The preceding 9bb4c97 receipt proves native preferences, InPrivate omission,
tray refusal/restoration, newer-copy races, cancellation/repeat, microphone
opening/preview deadlines, 100/150/200%, themes and Narrator keyboard navigation.
The 25a8f3b/8b0e469 receipts cover other provider fields and virtual display changes.

Original guest settings/history/last-result hashes were restored and the guest
shut down. No host pointer, keyboard, clipboard, installed app or user data was
used. Chrome/Firefox, physical microphones, USB/Bluetooth/sleep, long capture,
HDR/cable hotplug and ARM64 hardware remain outside this guest proof. Edge is
the authorized M4 browser gate. No benchmark cohort or intermediate CI push ran.
