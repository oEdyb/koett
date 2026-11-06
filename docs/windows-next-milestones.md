# Windows milestones after reliable dictation

M2 (#78) merged as dca0252e8fc8a3cc232e24b94315eb89384fc22f. M3 #113 merged as 8344c545503ddf76f54a62023993f74a1f8020d3 after six green
checks on 5bdc961 and real Windows software acceptance. Main's v0.3.0 source freeze remains 41fca8e;
this branch has not changed the frozen release or installed host apps.

## M3 — words and records

The [final c7e2376 receipt](../CrossPlatform/evidence/windows-milestone3/ci-c7e2376/README.md)
links the per-artifact evidence, including historical failures and their rechecks.
Screenshots include foreground HWND/PID and executable hashes. The guest is
bounded to 4 vCPU / 8 GiB, nice 19, and shut down outside active testing.

Passed software gates:

- Full-search History export (110 matches from 50 visible rows), raw/final Copy,
  selected/filtered export, native-file cancel preservation and legacy raw fallback.
- Native dictionary CRUD, canonical plain-text import/export, validation, explicit
  acceptance/cancel, denied writes, UIA selection, atomic rename/collision handling,
  stale-editor preservation and literal replacements applied once before delivery.
- Fix a word stores corrections separately from immutable history. Strict UIA
  Save/Add/Dismiss, everyday warning/Add anyway, rapid-selection protection and
  denied-write preservation passed without implicit dictionary adoption.
- Native CPU model switching across Compact/v2/v3/Nemotron with the same small
  prerecorded WAV, exactly-once Notepad delivery, v2 restart persistence, cancelled
  setup and settings-write rollback. The Nemotron success log has no fallback marker.
- Optional S1-mini controls, private CPU preparation, typing-focus preservation,
  successful formatting, original-ASR history, missing runtime/setup cancel/settings
  failure and formatter-crash raw fallback. Windows Job cleanup reaps the child
  after forced parent termination. A rule deleting the entire result keeps clipboard.
- Native UIA labels, forward/backward keyboard access with Narrator running, and
  all three new windows at actual 100/150/200% display scaling in light/dark.
- Windows executed 231 tests (12 ignored), including the same small dictionary,
  adoption and learning fixture consumed by Swift. Linux native 431 tests (22
  ignored), strict Clippy, release, formatting/diff and inventory/builder/pin checks
  passed. No Mac/AppKit SDK verification was performed on this Linux host.

Defaults remain Compact and Raw. No slow second encoder pass or big benchmark
was introduced. Shared product changes beyond Windows modules are limited to
Windows Nemotron admission in model.rs/runtime.rs and Windows-only console/Job
handling in formatting.rs, plus the Windows namespace feature in Cargo. Linux
and Mac app behavior is unchanged; no host installation/data was modified.

Limits remain explicit: physical mic/USB/Bluetooth/sleep/30-minute live capture
from M2, broad Swedish accuracy, injected Nemotron-specific recovery faults,
independent transcription of Narrator speech, and the plan's separate CTC acoustic
corpus gate. Focused WAV checks do not establish those claims. The optional
formatter used a pinned self-contained private CPU runtime (see runtime-static
receipt); bundling/signing/clean-install are distribution work in M6. M3 completion
is not a three-platform release or complete parity claim.

## M4 — draw and pin

- Use a read-based image-to-text gap around 200 ms. A timer after SendInput is
  not proof that an application read the clipboard. Preserve sequence/ownership
  checks so staging cannot overwrite a newer copy.
- Physical keyboard input only. The draw key must be seen up before arming;
  another key ends/spoils the hold until release. Check injected keys, startup
  key-down, normal typing/chords and rapid transitions.
- Pill-click stop retains the original HWND/process without restoring a
  different target. Silent recording preserves all clipboard formats.
- Preserve native PMv2, bounded image capture, own-window exclusion, per-display
  geometry and explicit missing context. Run the plan's privacy, mixed-DPI,
  pass-through, format and wrong-target checks. Missing context stays absent.

These are requirements carried from the reported Linux lessons, not Windows
passes. Physical device/display/sleep limitations remain separate from guest
or fixture evidence. M4 is now in progress on windows-draw-pin, based on the M3 merge; see [M4 acceptance](windows-draw-pin.md).
