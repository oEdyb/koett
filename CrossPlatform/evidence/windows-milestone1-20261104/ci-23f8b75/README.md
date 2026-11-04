# Windows 11 retest — CI head 23f8b75

Milestone 1 desktop foundation is complete against the agreed final gate:
UIA recovery Paste works without foreground activation. No fallback was needed.

## Artifact and environment

Unmodified artifact 13362504947 from [run 41911630406](https://github.com/oEdyb/koett/actions/runs/41911630406),
branch head 23f8b75f8e8a7651d7fe70218b603ed4a9ff02c5, tested PR merge
7d1d77e116884931e148cf85195ba2ac411871f9. The unsigned build receipt is included.
ZIP SHA256: 78d0f01a423889ea913f29c4208df375beec2bf16574c6b77dec5e56f5a5a814.
EXE SHA256: fdb8206d8a96f6db4d424d1fda7ca80c3e77c3225efe331038c1055a9283e19e.
Windows 11 Enterprise Evaluation, KVM, 4 vCPU, 8 GiB; 42 GiB host memory
available before boot, no own heavy builds. Private guest-only control.

## Real guest results

- Launch: PID 7316, 300×48 logical pixels at 200% (600×96 physical pixels).
- UIA Pause, Resume, Cancel, Copy and Paste: passed. Copy returned exactly
  `Desktop lab result`; Paste inserted exactly `Desktop lab recovery` once
  into a verified empty editor. Every sampled foreground was editor HWND
  131582 / PID 2708, including immediately before and after Paste.
- Done hover hold, hover exit, copied timeout and persistent recovery: passed.
- Switched-target Paste refusal: passed. Original editor retained its exact
  text, second editor remained empty, foreground stayed HWND 131628 / PID 9028.
- Real guest mouse actions: passed, including exact Paste and unchanged focus.
  The first mouse attempt stopped at a harness precondition: SendKeys failed
  to clear the isolated Edit control. The harness now clears that control via
  WM_SETTEXT, verifies emptiness, then tests the actual app Paste. It does not
  modify the app or bypass its delivery path. UIA was repeated with that harness.

JSON records include foreground HWND/PID per screenshot. Cropped PNGs keep
repository evidence small. Earlier ci-e63e65a and ci-7a0e827 receipts cover
startup off/on across boots, tray/restart, light/dark Settings, History Unicode
Copy/Export, reduced motion, Narrator and 100/150/200% scaling. Those checks
were not all repeated on this artifact. A dedicated virtual-action Narrator
speech/retained-provider teardown and standard-tooltip visual retest were not
run here; UIA Invoke was exercised directly. No claim of a full screen-reader
certification is made. The final user acceptance allowed M1 completion when
this UIA Paste gate passed.

The branch was rebased onto current main; Windows runtime source remains
identical to the tested artifact. CI reruns on the updated branch independently.
This desktop-lab run uses silent fixtures, not microphone/ASR. Real dictation,
media, drawing and complete cross-platform feature parity remain later work.
The guest was fully shut down after testing; no host installation/data changed.
