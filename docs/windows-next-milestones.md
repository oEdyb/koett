# Windows milestones after reliable dictation

M2 software gates now have real Windows receipts and #78 is ready for review.
Merge remains a separate explicit action. M3 is stacked on #78 until it merges;
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

## M3 first implementation checkpoint

Native History now offers Export results, independent of the visible 50-row
page. It captures the current search, reads successive cursor pages on the
existing history worker, and reports the exported count in the window. A native
Save dialog supplies the path; cancel leaves disk untouched. Export is bounded
at 8 MiB with an explicit error asking for a narrower search. A same-directory
part file is synced and renamed so a failed write preserves an existing export.

Focused coverage extends the existing 110-row paging test to full, filtered and
empty export and replacement of an existing output file. Windows metadata
Clippy passes; linked tests and real guest export/cancel/failure/UIA/scaling
checks still need the CI artifact. No guest runtime claim for M3 yet.

This is a Windows-only first checkpoint, not M3 completion. Dictionary editing,
Fix a word/acceptance, replacements, formatting/raw fallback and model switching
remain open. Do not expose the current slow dictionary decoder path. Use small
focused fixtures and timing checks; no big benchmarks.

Native History also has an explicit Raw transcript checkbox. Copy, single export
and full-search export use the selected view; rows without stored raw text fall
back to the final transcript. No new ASR or formatting work runs to view history.
Focused tests cover final/raw selection and legacy fallback. Metadata checked;
linked tests and guest UI checks remain pending the rebuilt Windows artifact.

## Current native Words implementation

Tray Dictionary and replacements opens native controls for entries, term/phrase,
sounds-like/replacement text, Save and Remove. Dictionary import reuses the
bounded shared text/JSON parser, merges only after explicit acceptance and uses
a native picker. Export writes the canonical text format atomically. Editing
uses Settings::edit, preserving unrelated concurrent settings. No model is loaded
by the window. Current main chooses shared-encoder decoding only when supported;
CTC uses spelling defaults. No experimental second pass is enabled here.

History export/raw/copy/filter/cancel/legacy fallback now have real guest evidence
under evidence/windows-milestone3/ci-34d2dbe. The native Words UI and the small
History search/list spacing correction are newer and still require their linked
artifact, guest interaction, accessibility and scaling checks. Fix a word,
explicit suggestion acceptance, formatting/raw fallback and model switching
remain open; M3 is not complete. #78 merged as dca0252; #113 targets main.

History Fix a word now reads the explicit UTF-16 selection from the native
transcript control and opens Words with that heard form populated. Nothing is
saved until Save entry. Missing/invalid selections show a local status message;
an existing Words edit is left intact. Focused tests cover emoji boundaries and
invalid selections. This implementation is compiled only until its guest run.
Optional formatting/raw fallback, model switching and remaining suggestion
acceptance/corpus checks are still open. No performance benchmark was added.
