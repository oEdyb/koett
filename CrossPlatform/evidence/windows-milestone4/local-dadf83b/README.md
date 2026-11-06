# Partial M4 guest receipt

Executable SHA-256: `dadf83be2cbf6c93e4194d29775af6e9789037c6e9333262899364cbeb8965ca`.
Locally linked with the explicitly approved xwin CRT/SDK and the pinned native
ASR libraries. This is a development build, not a release artifact. It predates
the microphone hotkey-retry fix and does **not** establish M4 completion.

`receipt.json` contains each native/desktop result and foreground HWND/PID.
The focused Windows test executable reported 14 passed tests and no failures;
PowerShell's Start-Process wrapper lost ExitCode after exit, so this receipt
records the actual test-runner output instead of inventing an exit code.

- WAV audio traversed the microphone-worker IPC path and real CPU ASR.
- Two Shift holds captured coral strokes, and a third captured a native-editor pin.
- Notepad HWND 787338 / PID 6844 stayed foreground through drawing and delivery.
- Clicking pill Stop while Shift was held ended recording. Text appeared once.
- Notepad did not request an image. The persistent Not pasted / Paste / Copy
  state remained visible; the text was still saved and delivered.
- UIA Copy exposed PNG, DIBV5, HTML and Unicode. `contact-sheet.png` is the actual
  copied PNG: three ordered frames, 1287 × 1211 pixels. Native/managed reads
  subsequently confirmed the complete Unicode text too. The initial managed
  read immediately after materializing PNG returned empty and is retained.
- Settings fit the 200% / 2560 × 1600 guest desktop. Other scales remain open.
- Original settings, History and last-transcript hashes matched after restoration.

Images are crops or reduced versions of private RFB captures, except Settings
(the unoccluded native capture) and the unmodified copied contact sheet. No
host desktop, pointer, keyboard, clipboard or installed Koett data was used.

The first stroke's underlying Notepad File menu was already open because the
activation helper clicked its menu row; it is not evidence of a canvas leak.
The next stroke closed that menu before drawing. Fast virtual mouse movement
also coalesced the initial position; gesture checks must pause after positioning.
Full click-through/capital/chord/undo/lock/cancel, browser, mixed-display,
rotation/hotplug and accessibility checks remain separate gates.
