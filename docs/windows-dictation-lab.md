# Windows reliable dictation — M2

This work starts from main after PR #52 merged as
fcaffcc9893c1d9bbf5d1e2ed5af3cc1b4f115b9. M1's four review fixes and ordinary
Paste/changed-target regression passed on the real guest; see its final receipt.
M2 software gates passed in the private Windows guest; physical hardware limits remain.

## Current checkpoint

The 1fa2169 artifact passed real Windows WAV-fed hold, pause/resume/cancel,
exact-once recognized text in Notepad/Edge/Windows Terminal, foreground-switch
recovery and read-only-history fallback. [Runtime evidence](../CrossPlatform/evidence/windows-milestone2/ci-1fa2169/README.md)
records exact text, paste events, history counts and HWND/PID.

The 03a9041 artifact passed the elevated-target and combined-failure retests,
plus storage fallback exhaustion with an honest Not saved label and Copy/Paste.
[Final recovery retest](../CrossPlatform/evidence/windows-milestone2/ci-03a9041/README.md)
records the results and readiness audit. M2 remains draft because software fault
gates (including ASR failure/retention and busy storage) remain unverified; these
are not hardware blockers. M3 starts after those gates close.

## First checkpoint

Native Settings now offers the persisted CPAL endpoint ID, System default,
Refresh and an explicit Preview/Stop preview action. Enumeration and preview
capture run on a worker. Preview uses the existing shared level conversion;
recording acquires the existing shared priority guard before opening a stream,
and preview releases the endpoint when recording takes priority. Closing
Settings closes preview. No model or file is loaded for preview. Unavailable
saved IDs remain visible rather than silently becoming System default; the
shared engine retains its existing recording-start replacement policy.

The original microphone controls have guest evidence above; newer hold/layout
changes still require their rebuilt artifact. They reuse the native [combo-box selection contract](https://learn.microsoft.com/en-us/windows/win32/controls/about-combo-boxes).
The Settings HWND remains native; the window is 560×440 DIPs and fitted to the
monitor work area. Verify labels, keyboard/UIA access, dark/light and 100/150/200%.

The Windows CI archive now also includes the existing `koett-engine.exe` CPU
WAV CLI. The same strict clean-system DLL gate applies to both executables.
It is diagnostic tooling; it does not route CLI output through desktop delivery.

After extracting the new artifact in the private guest:

```powershell
# Pinned model and official prerecorded sample; assert expected words, no mic.
.\koett-engine.exe --self-test
# Multiple mono PCM WAV fixtures, one shared CPU model load.
.\koett-engine.exe C:\path\to\model speech.wav silence.wav
```

Record the artifact/EXE hash, model ID, exact fixture hash, recognized text,
exit status and timing. Self-test may download the pinned model and sample;
a repeat with network disabled is needed to establish offline behavior. A
CLI WAV pass proves CPU recognition, not WASAPI capture or desktop paste.
Do not use `--desktop-lab` silent text fixtures as ASR evidence.

## Remaining M2 gates

- Shared CPU recognition: official WAV + silence, repeated offline recognition.
- Selected endpoint + actual level preview; default/explicit/unavailable selection,
  save/reopen, preview/record contention, no-sample versus silent input.
- Toggle and hold, silence/too-short, pause/resume/cancel, hotkey conflicts.
- Real recognized WAV text delivered exactly once to Notepad, a browser editor
  and a terminal; verify target buffers and one history row, not SendInput count.
  A controlled virtual input can feed the real recorder; if WAV injection is
  added, it must reuse session cancellation/delivery and be explicit lab-only.
- Change foreground during ASR and during modifier wait; destroy target;
  verify persistent Copy/Paste rescue and both-app buffers remain unchanged.
- Elevated target: standard-user Koett must show clipboard/manual-paste recovery,
  not elevate itself. Do not authorize a UIAccess helper here.
- Missing/disconnected input, callback failure/no samples, unavailable model/ASR
  failure, history write failure and fallback exhaustion. Verify audio retained
  only on ASR failure, transcript saved once on delivery failure, and storage
  never delays the paste event. Cancellation keeps neither audio nor transcript.
- Thirty-minute **live** capture before claiming long-session support.

The current QEMU audio backend has no real input source. Prerecorded WAV/virtual
input checks must be labelled separately. Physical USB unplug/default-device
switch, Bluetooth, privacy settings, sleep/resume and long live capture require
real Windows hardware and remain open. No claim about them follows from CI,
synthetic levels, fixture duration, or a seven-minute long-input CLI probe.

Guest budget remains 4 vCPU/8 GiB; check at least 12 GiB MemAvailable before
boot, stop own builds, control only private guest sockets, and fully shut down
after testing. Keep model/audio/disk/artifacts in the private cache, not git.

## Remaining fault-run checkpoint

`--dictation-lab-asr-failure FILE.wav` is an explicit Windows diagnostic mode.
It captures actual paced WAV frames, stops the ASR worker at finalization, then
uses the existing worker-error/audio-retention path. It does not invent a
transcript or claim that decoding succeeded. Restart normal WAV mode afterward.
`asr-retention-m2.ps1` checks readable retained audio, unchanged clipboard/editor
and history after the fault, then no extra retained audio after a successful run.

`busy-storage-m2.ps1` holds an exclusive SQLite lease using the guest's system
winsqlite3 test helper. It checks that target text appears before unlock and
before history changes, then that history changes after unlock. The lease has
a timeout and a finally-release path. It does not replace any system DLL.
`hotkey-faults-m2.ps1` leases the default chord, checks the tray warning and tray
record/cancel fallback, then releases it and verifies shortcut recovery after
restart. Both scripts require the isolated Notepad target and concatenated M2
helpers. These new checks are prepared, not guest passes yet.

Shared files changed for the diagnostic: `runtime.rs` and
`transcription/background.rs`. Every new fault branch/API is cfg(windows), and
normal microphone/WAV capture does not arm it. No main/release branch was edited.
Windows CI also parses the PowerShell checks before native compilation. The
local guest stays off while other builds consume the host's headroom.

## Software gate completion

[Final software receipts](../CrossPlatform/evidence/windows-milestone2/ci-72ca2ae/README.md) close ASR retention, busy storage, hotkey conflict, desktop silence/short/no samples, destroyed target and clipboard contention. Earlier draft/readiness paragraphs above are historical checkpoints. PR #78 is ready for review; hardware limitations remain as listed. No big benchmarks were run. M3 continues on a branch based on #78.
