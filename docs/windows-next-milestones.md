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

- Native Words supports bounded shared dictionary text/JSON import, canonical
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
