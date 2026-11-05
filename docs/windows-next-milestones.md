# Windows milestones after reliable dictation

M2 must finish its software gates and receive review before #78 is ready.
Merge remains a separate explicit action. Start M3 on a new branch from main;
keep Windows commits isolated from the v0.3.0 release and flag shared-file edits.
M4 follows the words/records milestone. The windows-port-plan remains the gate
list; this note carries the 2026-11-05 implementation constraints forward.

## M3 — words and records

Build native searchable history/export, dictionary import/export, Fix a word
with explicit acceptance, replacements, and optional formatting/raw fallback.
Reuse the shared data formats and fixtures. Do not enable or copy a second
encoder pass for dictionary bias. The shared loader currently accepts dictionary
settings, so audit that path before exposing Windows editing: a native UI alone
must not accidentally turn on the slow bias implementation. Follow the pending
single-encoder performance review and measure decode work/latency on the same
corpus. Keep spelling/replacements distinct from acoustic vocabulary support.
No default-model change or speculative dependency upgrade is authorized here.

## M4 — draw and pin

- Use a read-based image-to-text gap around 200 ms; do not mistake a timer after
  SendInput for proof that an application read the clipboard. Preserve the
  clipboard sequence/ownership check so staging cannot overwrite a newer copy.
- Accept the draw gesture from physical keyboard input only. It must be seen
  up before it can arm; another key ends a held gesture. Test injected keys,
  startup with the key already down, normal typing/chords and rapid transitions.
- Stopping by pill click must retain the original HWND/process and paste back
  there without focus restoration to a different target.
- Silent recording must preserve the clipboard, including staged image formats.
- Keep PMv2/native overlay behavior, bounded image capture, own-window exclusion,
  per-display geometry and explicit missing context. Execute the plan's privacy,
  mixed-DPI, pass-through, clipboard-format and wrong-target checks on Windows.

These are requirements carried from the reported Linux lessons, not Windows
verification results. Physical device/display/sleep checks remain separate from
QEMU or synthetic input evidence. Do not claim M3/M4 parity from compilation.
