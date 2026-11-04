# Milestone 1 — ongoing real Windows checks

## Current checkpoint

Milestone 1 remains **incomplete**. The green, unmodified CI artifact 7a0e827
passes the corrected waveform, dark/light checkbox toggle, compact Settings,
verified typing focus, visible states, reduced-motion visual checks, native
Settings/pill geometry at 100/150/200%, and scoped dark-checkbox Narrator
navigation. Earlier artifact receipts retain History Copy/Export and startup
on/off across boots. See [the latest retest](ci-7a0e827/README.md).

**UIA Invoke still failed** in 7a0e827: activation moved focus to the pill and
the original-target Paste guard refused. The earlier focus restoration was
insufficient and is removed. Source now exposes virtual UIA fragments through
the DirectComposition HWND. An isolated managed probe preserves foreground;
the new Rust implementation has Windows all-target metadata Clippy evidence
only and still requires its linked-executable test. Native Settings/History
remain native. The tray standard-tooltip correction also needs that retest.

Do not mark the PR ready until the rebuilt executable passes UIA exact Paste,
changed-target refusal, pointer/state, Narrator/status, teardown and tray
regressions. No original-target guard was relaxed. Root status names and a
revision counter cover accessible announcements and same-tick transitions.
Supply-chain outputs are current; six dependency tests and ten inventory tests
pass without a policy bypass. Native Linux tests report 389 passed, 17 ignored;
Clippy/release pass; six pure timing/state tests pass.

The guest completed a full Windows shutdown. No QEMU process or transfer server
is left running. Its disk remains private under ~/.cache/koett-windows; the
host Koett installation/data, Mac app and Linux product code were untouched.
No spend, license purchase, privileged helper, install or release was made.

The remaining sections preserve earlier investigation evidence and failures;
they are not the current pass list.


This is a partial receipt, not Milestone 1 completion. PR remains draft.

## Executed on Windows

Guest: Windows 11 Enterprise Evaluation, QEMU/KVM, 4 vCPU / 8192 MiB.
Initial host memory: 39 GiB MemAvailable; no own native builds active. Private
VNC/QMP/serial sockets only; no host desktop input. VM disk remains under
~/.cache/koett-windows/. Later display configuration uses 64 MiB virtual VGA
with preferred 2560x1600; guest RAM/vCPU caps are unchanged.

- Artifact 89234c1 launches as an ordinary user with VCRUNTIME140_1.dll absent.
  clean-launch.json records real PID/path/SHA and focus. Artifact ZIP and EXE
  hashes and the tested GitHub merge commits are in artifact-provenance.json.
- A real WinForms editor received typing while recording UI was active.
  All 20 foreground samples retained editor HWND 66352 / PID 9172. Initial
  text extraction in that test used GetWindowText on a foreign Edit control,
  which cannot retrieve its contents; that empty field is not text-loss evidence.
  The screenshot shows the typed phrase. This does not prove pointer pass-through.
- Overlay reports 300x48 at 96 DPI. Done window becomes hidden after timeout.
  Those geometry/timing observations do not establish visible rendering.
- Native History opens, searches an explicitly seeded lab-only canonical SQLite
  fixture, and pointer selection + accessible Copy returns the exact Unicode
  text and LF. history-pointer-copy.png/json capture the native UI and HWND/PID.
  The fixture is not an ASR result. UI Automation SelectionItem.Select initially
  left the detail stale; source now also observes native list selection on poll.
- Native Settings startup checkbox exposes CheckBox/TogglePattern, and Save
  removes/adds the correct HKCU Run entry. Saved off/on screenshots included.
- After a full shutdown/boot with startup on, Koett launches normally (no lab
  argument) under Explorer: PID 9788, parent PID 4904. The clean foreground
  remains Explorer. Later install of 31178ee explicitly quits that instance.

UI Automation test client initially lacked standard control proxies; registering
UIAutomationClientsideProviders after initializing RootElement restores standard
Text/Edit/List/Document/Button/CheckBox roles. This was a harness issue, not proof
that the native controls lacked accessibility. Source labels were also adjusted
so History list/document no longer inherit the Search field's accessible name.

## Real failure and narrowed cause

89234c1 pill HWND becomes visible but its actual recording/transcribing/Done
content is invisible. Cropped PNGs preserve that failure; do not present them as
visual parity evidence. Removing WS_EX_LAYERED at runtime alone did not fix it.

31178ee initializes COM, removes layered-window composition conflict and writes
bounded renderer diagnostics. The real guest reports:
`Pill initialize: No such interface supported (0x80004002)`.

A direct native DCompositionCreateDevice2 call in the same guest proves:
- IID_IDCompositionDevice2 -> 0x80004002
- IID_IDCompositionDesktopDevice -> 0x00000000

composition-interface-probe.json records both HRESULTs and focus. Microsoft's
API requires requesting IDCompositionDevice or IDCompositionDesktopDevice at
creation. Source now requests IDCompositionDesktopDevice directly. The newly
linked binary still needs visual retesting; native API success alone is not a
rendering pass.

https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nf-dcomp-dcompositioncreatedevice2

## Pending

Full pill states, hover/copy/recovery, click-through and rendered timing; tray and
Explorer restart; startup-off after login; dark mode and reduced motion; Narrator;
100/150/200% native window/overlay checks; final native History export and keyboard
checks. Guest initial display only exposed 100/125%; a larger virtual display
was provisioned for the required scales. Windows installed an automatic update
on the first shutdown; no forced termination was used.

Source refinements awaiting their linked-binary test include embedded common
controls/PMv2 manifest, DPI fitting, native theme colors, accessible heading
association, window reuse, exact stored text for Copy/Export, CRLF display-only
conversion and disabled result actions until a transcript is loaded.


## Final local validation

`final-local-validation.json` and the adjacent logs record native Linux
`cargo test --locked` (388 passed, 17 ignored, plus the isolated child test),
strict all-target Clippy, and a release build. Five meter and five timing/state
unit tests pass. Windows-target metadata Clippy passes with DOCS_RS/native-link
skips explicitly recorded. The final Windows fixes have **not** run in a newly
linked executable. Swift test/release checks were not run on this Linux host.
Supply-chain inventory remains current; 10 inventory and four PE dependency
validator tests pass. No policy was weakened.
