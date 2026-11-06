# Windows M4 foundation — real guest receipt

Executed source `03a7d0aad9fdfa9b64fa56aed0e125c1be44383d`, from
[Windows CI run 42042397523](https://github.com/oEdyb/koett/actions/runs/42042397523).
Executable and archive hashes, foreground HWND/PID and per-step results are in
[receipt.json](receipt.json). This is foundation evidence, not M4 completion.

- PNG, DIBV5, UTF-8 HTML and exact Unicode text round-tripped. Text staging and
  explicit repeat Copy succeeded. A newer clipboard copy blocked staging.
- WGC captured the real desktop. The visible pill is present in
  `framebuffer-visible-pill.png`, absent from `wgc-excluded-pill.png`; both crops
  were inspected. The 600×96 physical pill was at 192 DPI (300×48 logical).
- A QEMU keyboard Shift hold armed the device-associated Raw Input path. A
  second key spoiled the hold; release cleared it. SendInput Shift did not arm
  drawing. This is emulated-device evidence, not physical-keyboard attestation.
- While Notepad was foreground, those checks retained HWND 197356 / PID 4356.
  The newer-copy-during-WGC race retained the newer text and the same foreground.
- Guest Koett settings/history matched their original hashes at shutdown.
  Koett exited, Windows shut down, and guest_control reported not running.

The canvas, contact sheets, staged observed-reader paste, native pin context,
new Settings/Stop actions and isolated microphone process are newer than this
executable. None of those paths is proven by this receipt. Narrator, browser
context and mixed-display checks must be repeated on the final implementation.

The already-triggered build of `3d7e604` produced a Windows artifact; its guest
start was refused because the host was below the required 70% idle threshold.
No guest was started for that artifact and no further CI push was made.

The ARM64 job at `3d7e604` exhausted the existing three bounded attempts on an
ONNX Runtime asset HTTP 504, before Rust compilation. A direct HEAD recheck of
that exact asset returned HTTP 200 (`arm64-download-recheck.json`). No policy,
checksum or dependency pin was weakened; no code regression was shown by that
failure.
