# Partial M4: opening retries and Edge paste

Executable SHA-256: `bee18d537abb6ffbc0beaff4ede328f02ab46093a51614118314747da299df52`.
`receipt.json` includes the complete build receipt and foreground HWND/PID per
check. This is a locally linked development executable, not a signed release.

- Two default-input opening stalls ended visibly in 5153 / 5132 ms.
- Two named-input resolution stalls ended visibly in 5045 / 5149 ms.
- Every retry was a real registered hotkey event, observed a distinct worker PID,
  then left zero worker processes. Explorer foreground HWND/PID stayed unchanged.
  These are injected worker stalls; no physical microphone was tested.
- Edge HWND 458964 / PID 6736 stayed foreground through recording, two physical
  Shift pins and pill Stop. Real CPU WAV transcription completed.
- Edge received exactly two paste events: one image, then text. The image had
  no text payload; the text event had no image. The measured event gap was
  1527 ms, the bounded fallback, not the desired fast reader path.
- A selected public phrase was attached to its pin. A focused password pin
  omitted password text; the fixture password never appeared in the result.
- Chrome was not installed or tested. It is explicitly excluded as an M4
  blocker by the user's instruction; Edge is the Chromium paste check.

The address bar exposed a scheme-hidden value. The browser document itself
exposed the complete URL through ValuePattern. This receipt predates the fix
that reads that actual document value; its captured pin still says URL
unavailable. It also predates the added tray Paste action and clipboard-reader
trace. Those changes require rebuilt guest checks. It does not prove M4 complete.

`edge-paste-events.jsonl` is the receiver page's actual event log. The PNG is a
reduced private RFB screenshot. Clipboard text, API values and event counts are
preserved in the receipt. No host installation/data or host input was used.
