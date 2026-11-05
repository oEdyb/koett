# M2 remaining fault checks — real Windows

CI run 41951026303, source 72ca2ae4d04993ba922f2d297245ef8810ad4a1f.
ZIP SHA256 f4e52cf9febfaafdaF9bc02c1ae6f653c210a79745d1615ba0d5d303d995fa95.
EXE SHA256 f76957a65485ffcaee8333153f88ce9438e0f68e0a9928b34ebc40277a52c7e3.
Host had 40 GiB available, load 1.39 before boot. Private Windows 11 guest:
4 vCPU/8 GiB at nice 19. No own host builds. Official prerecorded WAV only.

All three scripts returned passed:true against this linked executable:

- ASR worker failure: one 61,484-byte RIFF/16 kHz WAV retained; visible error
  names the recovery path. Editor empty, clipboard sentinel and history unchanged.
  Normal successful recording then pasted the expected Phoebe paragraph; zero
  additional audio files retained.
- Exclusive history lock: recognized text appeared in Notepad in 123.3 ms while
  the separate SQLite lease remained held. History changed after unlock. The
  first harness attempt could not hash a Windows-locked file; corrected to
  check the live exclusive lease and compare pre-lease/post-unlock hashes.
- Hotkey conflict: reserved Ctrl+Shift+Space did not record. Explorer exposed
  the tray warning through its UIA accessible name. Tray record/cancel remained
  functional. After releasing the chord and restarting, key-down started and
  key-up ended recording. Guest config was restored and lease exited.

The tooltip itself was not exposed by UIA in this run; the assertion uses the
actual tray icon accessible name. Cropped screenshots and foreground HWND/PID
are retained per step. Recording phases were sampled before cancellation/release;
post-action screenshots can therefore show a hidden pill. No claim of a visible
hover tooltip follows from those screenshots.

This closes the three requested fault regressions. Earlier evidence covers
emulated mic/preview, CPU WAV, hold/pause/cancel, three-app exact-once delivery,
changed/elevated targets and combined storage failure. Physical microphone,
USB/Bluetooth transitions, sleep/driver behavior and 30-minute live capture
remain unverified. Additional software edge cases listed in the prior audit
(desktop silent/short/no samples, destroyed target, clipboard contention) have
not been newly run here and are not hardware limitations.
