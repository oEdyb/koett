# M2 elevated and combined-failure retest

Unmodified head 03a904137684085c525f23fb6bdcf46888ce01b3; run 41933126507,
artifact 13373936423; tested merge 7d567151611bea889bad7583950296f17138d7a4.
ZIP SHA256 3c344cd8488d71ba6c51e1f8c49e0e2929e6892c2fea6c9e3b1eb69a83439f80.
EXE SHA256 ffddb686ab54a710d1ab389736683725f3e9d462be0e346afdce482720abd1d5.

Host 40 GiB MemAvailable, sampled 91% idle. One Windows 11 KVM guest,
4 vCPU/8 GiB; guest threads verified nice 19. No concurrent own builds.
The actual CPU engine used the official prerecorded WAV through
--dictation-lab-wav, not synthetic desktop-lab result messages.

All three real Windows scripts returned passed:true:

- Elevated editor: Koett integrity 8192, editor 12288 (administrator). Editor
  remained empty. The pill now said Not pasted with Paste and Copy, and Copy
  returned the recognized transcript. Foreground HWND 197518 / PID 5520.
- Combined changed-foreground/read-only-history failure: persistent Not pasted.
  Saved with Paste and Copy. Foreground HWND 131914 / PID 4004. Readonly restored.
- Storage fallback exhaustion: primary DB read-only and fallback directory
  temporarily replaced by a file after preserving the original directory.
  The pill said Not pasted. Not saved. Copy to keep, with both actions. Copy
  returned the transcript; DB attribute and recovery directory were restored.

Screenshots and foreground HWND/PID accompany each step. The new integrity
preflight and storage-aware recovery fixes therefore have linked-executable
Windows evidence. No elevated Koett/helper, host input/data change, merge or
release. Guest fully shut down; both local transfer servers stopped.

## Readiness audit

PR remains draft: passing these regressions does not complete all M2 software
fault gates. Earlier receipts cover hold/pause/cancel, three-app exact-once
paste, foreground refusal/retry, emulated input selection/preview and offline
CPU speech/silence. Still open on the real desktop: ASR finalization failure and
retained-audio recovery, busy storage/delivery timing, hotkey conflict recovery,
desktop silence/too-short/no-samples, destroyed target and clipboard contention.
Those are software checks and are not relabelled as hardware limitations.

Real hardware is needed for physical microphone fidelity, USB/Bluetooth device
loss/change behavior and a 30-minute live capture. Physical sleep/wake driver
behavior needs hardware; guest privacy and sleep checks can still run in the VM.
M3 (words and records) follows only after M2's remaining software gates close.
