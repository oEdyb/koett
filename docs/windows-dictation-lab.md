# Windows reliable dictation — M2

This work starts from main after PR #52 merged as
fcaffcc9893c1d9bbf5d1e2ed5af3cc1b4f115b9. M1's four review fixes and ordinary
Paste/changed-target regression passed on the real guest; see its final receipt.
M2 is **in progress**, not a dictation-readiness claim.

## Current checkpoint

The ae8e454 artifact passed real guest endpoint persistence/default selection,
preview stop/close/reacquire, unavailable-endpoint refusal and offline CPU
speech/silence WAV recognition. See [the receipt](../CrossPlatform/evidence/windows-milestone2/ci-ae8e454/README.md).
These used emulated input, not a real mic. M2 remains incomplete.

New source adds hold mode, queued cancellation, original HWND+PID target guards,
and an explicit WAV-fed desktop path:

```powershell
.\Koett.exe --dictation-lab-wav C:\Users\KoettLab\speech.wav
```

This loads the shared CPU engine normally. Use the actual shortcut or tray to
start/stop. The feeder supplies real 16 kHz PCM frames at 20 ms intervals;
pause discards frames and cancel wakes the engine. It reuses normal session IDs,
ASR, delivery and history. It is distinct from --desktop-lab, which only presents
silent UI fixtures. Source WAV remains a lab input, not failed recording data.
WAV input is capped at 20 MiB and ten minutes. This new path, hold controls and
reentrant theme fix need a rebuilt-executable guest retest before any pass claim.

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
The Settings HWND remains native; the window is 560×400 DIPs and fitted to the
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
