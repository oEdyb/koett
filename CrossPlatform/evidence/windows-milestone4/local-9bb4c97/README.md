# Local Windows M4 verification — 9bb4c97

Application SHA-256: `9bb4c97cbbe9672268c3095371306e579fbbb8ae159038c849f8d0858c39bca0`.
Locally linked MSVC development build, rebased onto main d7c8ec4; not a release.
`assertions.json` validates 20 assertions against the raw guest receipt.

Passed on real Windows:

- 20 targeted native tests exited 0, including taskbar ancestry/process checks,
  private-label descriptions, delayed clipboard restoration and duration bounds.
- Default and named microphone opening each timed out visibly twice in about
  five seconds, with hotkey retry and no workers left. Device enumeration and
  two preview retries also timed out cleanly; duplicate commands did not queue
  extra workers. Normal virtual-HDA preview started and stopped with no orphan.
  Physical microphones were not tested.
- Native paste-menu UIA choices saved text, default-image and per-app shortcuts,
  and restored inheritance. A separate physical guest Escape cancelled the
  root menu without a preference change. Injected Escape was replaced by this
  actual QEMU keyboard check in the harness; no cancellation claim relies on it.
- Tray recovery inserted one Desktop lab recovery in Edge HWND 262298/PID 8472.
  A refused paste returned to the other editor HWND 393914/PID 6020, changed no
  clipboard/history data and showed Not pasted. The authenticated parent chain
  fixes Windows 11 InputSite windows inside the Explorer taskbar.
- Physical-key drawing created two DPI-aware canvases (168/96 DPI), plus an Edge
  InPrivate pin. A physical click on the pill's Stop while Shift was held ended
  capture; release preceded delivery. One image and one text event reached the
  original Edge HWND 197566/PID 8472, 319 ms apart, without focus theft.
- The InPrivate pin omitted title, URL and selection, and explicitly said
  Private mode: context omitted. Automatic completion retained PNG, DIBV5,
  HTML and Unicode without Copy. The actual clipboard PNG is included.
- A copy after text consumption won. Cancel in another editor preserved
  clipboard/history/last-result hashes. Returning to the original editor then
  repeated exactly one image/text pair without another history write.
- New controls fit and exposed native names at 100/150/200%. Screenshots were
  inspected. Light/dark toggles, recording-limit Save at 120 and 60 minutes,
  Narrator running with named keyboard navigation, and normal preview passed.
  Narrator was turned off using the physical guest shortcut; speech quality
  was not graded. No long recording or benchmark was run.

One early repeat assertion wrapped Windows PowerShell's JSON array an extra
level; the corrected check measured six events becoming eight and passed. The
live History hash uses a read/share-write stream because Settings holds a SQLite
connection; the app itself uses DELETE journaling. These were harness repairs.

Guest settings/history/last-result hashes and 1920x1080 + 1280x1024 / 175% layout
were restored before shutdown. Guest wall-clock UTC differs from host UTC;
relative timing uses monotonic clocks. Only guest input was controlled.
Chrome and Firefox remain untested; Edge is the authorized browser gate.

The next build adds the final Mac 450 ms pin pulse/reduced-motion behavior and
moves foreground validation after clipboard validation. Those later changes
need their own focused tests and guest receipt; they are not proved by this one.
