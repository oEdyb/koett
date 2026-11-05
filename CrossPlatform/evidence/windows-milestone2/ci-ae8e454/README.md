# M2 first real Windows checks

Unmodified CI artifact ae8e4548b2a357fa77195bbbab6e9db2bc2bc25d, run 41921968476,
artifact 13367903814, tested merge bdf0056d8cce7d7877202933a5750f0019a1ace2.
ZIP SHA256 f1baa21b43656d6b690ed1b0cc1f64c0c56ae385da8b1b5f6361a3bf898fba53.
Desktop EXE fb87d2556320d346cfb710e51c864606f671fd28d7b072e3b59fb1b611885e15.
CPU CLI 19722033271df552e1306be0cb6964c51b2af80481411d6b968cfbcfd22e6233.

Host initially 41 GiB MemAvailable and about 90% CPU idle. Guest 4 vCPU/8 GiB.
No host input or installed Koett data touched. Guest is powered off and transfer
server stopped. Physical microphone checks were not performed.

Passed in the guest:
- Emulated HDA endpoint ID selection, Save/reopen, System default persisted null.
- Preview received silent level callbacks, stopped, closed and reacquired input.
- Unavailable selected endpoint stayed visible and Preview refused fallback;
  the original lab settings were restored exactly.
- CPU self-test asserted Phoebe/old portrait, 7.435 seconds decoded in 140.3 ms.
- With QEMU network link n0 disabled, CPU CLI recognized speech and returned
  empty text for three seconds of silence, exit 0. Final run: model load 1601.2
  ms, speech 138.7 ms, silence 52.0 ms. Timing is this VM only, not hardware
  performance evidence. Fixture/EXE hashes and foreground HWND/PID are in JSON.

The original asynchronous CLI launch did not capture exit status; the offline
retest explicitly retained the process handle and captured exit 0. The serial
bridge stopped responding during collection; resetting its standard-user task
recovered it. Requests are now paced and the bridge timeout is 30 seconds.

The host scheduler reset ordinary QEMU to nice 9 despite nice 19 launch.
The offline CPU run was checked at nice 19 before and after. A byte-identical
private cached QEMU alias, explicit system ROM/module paths and private process
name now retain nice 19; verified on a full Windows boot after scheduler scans.
No host scheduler configuration changed. Starts now refuse less than 70% sampled
CPU idle or 12 GiB available. The final guest run and transfer server are off.

Screenshots exposed mixed dark/light Settings painting during reentrant layout.
Source now caches paint colors independently of the mutable window state.
New source also adds hold mode, explicit --dictation-lab-wav input through the
shared recorder/session/CPU engine/delivery, PID+HWND target identity, original
start-target retention, and immediate queued Cancel. These changes have not run
as a linked Windows executable yet. The WAV feeder is bounded to 20 MiB, 16 kHz,
ten minutes; pause drops frames, cancel stops feeding, source WAV is retained as
a test fixture. Unit tests cover exact samples and paused/cancelled omission.

M2 remains incomplete: retest those changes, then exact-once recognized text in
Notepad/browser/terminal, hotkey conflicts, ASR/clipboard/history failures and
foreground changes. Physical USB/Bluetooth, sleep/privacy and 30-minute live
capture need real Windows hardware. No readiness or all-platform claim.
