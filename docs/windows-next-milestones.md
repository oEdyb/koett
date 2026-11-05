# Windows milestones after reliable dictation

M2 (#78) merged as dca0252e8fc8a3cc232e24b94315eb89384fc22f after its software
gates passed. Hardware limitations remain explicit in its receipts. M3 #113
now targets main and stays draft; no M3 merge before separate approval/source
freeze. Windows product work must not disturb Mac/Linux v0.3.0.

## M3 — words and records

Real Windows evidence for green 34d2dbe is in
`CrossPlatform/evidence/windows-milestone3/ci-34d2dbe/`: all 110 search matches
exported from 50 loaded rows, raw/final Copy, single/filtered export, native
Save cancel preservation and old-row raw fallback. Guest history was restored
with a matching hash and Windows shut down. This is not complete M3 proof.

Implemented Windows-only candidates:

- Native Words supports bounded shared plain-text dictionary import, canonical
  text export, explicit Save/Remove, and literal phrase replacements. Imports
  merge only after acceptance. Settings edits preserve unrelated changes.
- History Fix a word now edits a transcript correction in the shared Corrections
  store. It does not rewrite history. Learning runs on a worker; proposals need
  explicit Add or Dismiss. Everyday forms warn and require Add anyway. The
  earlier selected-phrase shortcut is superseded by this shared-store flow.
- A separate native Model and Formatting window keeps Settings compact. It uses
  existing engine commands for 110M/v2/v3/Nemotron and all S1-mini controls.
  Requests are rejected while recording/setup is busy. Progress/errors/cancel
  are visible; only the engine commits a successfully prepared model/formatter.
  Failure preserves the working configuration; raw remains the default.
- Existing main selects shared-encoder transducer decoding only when the native
  patch is supported; CTC defaults to spelling. No experimental second encoder
  pass is exposed. No model defaults, dependencies or Mac/Linux runtime changed.

Metadata Clippy, formatting/diff checks and normal native tests pass. New native
UI, correction store/proposals, model switching/cancellation and S1-mini
success/raw fallback still need their linked executable and guest evidence.
The Windows tests now consume the same small Swift dictionary/adoption/learning
fixture; the new formatting command test covers every wire option. Those tests
are compiled locally; Windows CI executes them. Do not substitute metadata
compilation for Windows execution.

Remaining gates: guest Words import/export/edit/remove/validation, explicit
correction Add/Dismiss/everyday acceptance, settings rollback/write failure,
model switching/default preservation, formatting success/failure/raw fallback,
UIA/keyboard/light-dark/scaling, and a small speech/near-miss/timing check. Keep
the CTC acoustic gap explicit. Use focused fixtures and isolated measurements;
no big benchmarks. The optional Windows formatting lab uses official llama.cpp
b12125 CPU binaries, SHA256
968091ccb8c230e0cf7e0eed386310703bebae4a4fd4375f0c57db53958307d8,
only in the private guest cache; this is not a bundled runtime dependency.

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
or fixture evidence. M4 follows completion of M3.

## Latest regression fixes awaiting linked retest

The 6165f3f guest passed correction storage, explicit everyday acceptance,
Dismiss and denied-write preservation through keyboard selection. Pure UIA
selection revealed stale labels, now fixed with the same selection observation
used by History. Word category/selection observation was fixed too. Editing a
selected entry now renames it atomically, rejects rename collisions or stale
snapshots, and offers New entry. Literal replacement whitespace is preserved.
Focused tests cover rename, conflict/validation preservation and whitespace.

One shared Rust file now has Windows-only changes: formatting.rs suppresses the
formatter console and retains a kill-on-close Windows Job. The job implementation
is Windows-only and Cargo adds only its Windows namespace feature. Mac/Linux
paths and defaults are unchanged; normal native tests/Clippy/release pass.
A Windows child-lifetime test compiles and awaits execution in Windows CI.

The official llama.cpp CPU download needed VCRUNTIME140.dll on the clean guest.
A private test runtime built from pinned MIT source with the existing Zig
compiler has only system imports and passed --version on Windows; no runtime
installer/licence decision, host install or global PATH change was needed.
See the runtime-static receipt; binaries/models remain outside git. This is
not a bundled dependency or completed formatting end-to-end proof.

The 6165f3f guest also passed Compact and v3 CPU WAV-to-Notepad delivery,
S1-mini preparation/formatting and formatter-crash raw fallback (once, same
foreground HWND/PID). V2 delivered once but a spelling-specific harness
assertion needs a corrected re-run. Receipts are under ci-6165f3f. A remaining
shared model/runtime platform guard prevented Windows Nemotron despite the
shared CPU recognizer; it now admits Windows too, with Linux/Mac behavior
unchanged. This additional shared-source change requires linked Windows
execution before claiming Nemotron support. The default remains Compact.

On green b964d47, strict UIA correction selection/acceptance and Words
category/selection/rename/collision/stale-edit all passed. A forced parent crash
also reaped the Windows Job's formatter child within three seconds. Model write
denial rolled back, but cancellation left a stale Loading model status despite
preserving settings; the Windows presentation now reports cancellation explicitly.
Native UIA inspection found Style was associated with the Structure label;
label/control creation order is corrected. Both fixes await linked re-run.

The live CPU WAV word test caught a missing Windows delivery stage: dictionary
spelling applied and raw history survived, but replacements were never applied.
Windows now applies shared literal rules once, after cleanup/dictionary and
before clipboard/paste/history. It keeps original ASR text and reuses the
existing empty-result clipboard-preservation guard. A focused Windows test
covers original/raw preservation, no cascading and empty output. Linux/Mac code
is untouched by this fix; the linked WAV/empty-result re-run remains required.

Additional guest gates passed: full-search History export/raw Copy/legacy
fallback re-run, missing formatting runtime, denied formatter settings writes
and setup cancellation, all with preserved Raw mode/child cleanup. Narrator
navigation found multiline EDIT consumed Tab; native form navigation is now
explicit for Tab/Shift+Tab (other editing/modifier keys keep native behavior).
A rapid UIA selection could adopt an everyday form before its warning refreshed;
Add now requires the current warning to have been presented. Native Windows
regression tests cover both fixes; linked execution is still required.

Post-main native verification: 431 tests passed, 22 ignored, strict Clippy,
release build, formatting/diff and 10 inventory/16 native-builder/8 workflow-pin
checks passed. Windows all-target metadata Clippy passed for the latest fixes;
this is compilation, not execution. Swift/AppKit SDK checks are not available
on this Linux host. No physical hardware or broad accuracy benchmark is claimed.
