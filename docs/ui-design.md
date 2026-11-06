# Koett desktop UI

> Historical evidence and commands use the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). See [the evidence archive](evidence-archive.md) for private reproduction.

Approved scope: Linux port and UI first, searchable history on Linux and macOS.
Olle explicitly found the previous interface unintuitive. This document governs
the implementation; visual polish must support observable tasks.

## Visual redesign brief — 2026-10-28

Olle accepts the broad control set and list/detail layout, but explicitly rejects
the current visual quality. Restyle the actual native History and Settings screens,
including empty/error/narrow states. Another menu-only pass does not meet this brief.

References inspected: the saved approved [shadcn/ui style](https://styles.refero.design/style/0fd67ec5-7e9c-4ca9-b368-5d9c7388477a)
for restrained controls/type hierarchy, [Seline style](https://styles.refero.design/style/7967c6d9-e50c-42b5-b4d1-74003ba41781)
for spacing and surface hierarchy, and [GNOME boxed lists](https://developer.gnome.org/hig/patterns/containers/boxed-lists.html)
for grouped native preferences. These are composition references, not source code
or image assets to copy. The local UI reference index is absent on this machine.

Use a small GTK-compatible semantic theme, scoped to Koett's windows. Raw palette
values feed roles for canvas, surface, selected surface, text, secondary text,
border, primary action and focus. Light: canvas `#FAFAF9`, surface `#FFFFFF`, text
`#1C1917`, secondary `#75706B`, border `#E7E5E4`. Dark: canvas `#181716`, surface
`#242220`, text `#FAFAF9`, secondary `#A8A29E`, border `#403C38`. The existing coral
identity remains: primary action `#B84236` with white text (5.43:1 contrast).
Honor light/dark changes, native font scaling, visible focus and high contrast.
No new fonts/dependencies, gradients, heavy shadows, decorative cards, invented
statistics, AI titles or explanatory marketing copy.

Keep native system type. Use a clear 20px-equivalent window/section title,
14px-equivalent list excerpts and controls, 12px-equivalent metadata, and
16px-equivalent transcript text, scaling with the system font. Follow a 4/8px
spacing rhythm, 24px reading/settings padding, 8px control radii and roughly
300px history navigation. Timestamp and excerpt are distinct row elements.
The reading pane has its own heading/metadata and compact Copy/Export actions.
Settings retains its four sections, with grouped rows and a persistent Save area,
without the large unused gray body. Empty states carry Koett's identity quietly.
The live coral waveform pill stays unchanged as the signature element.

Acceptance: old/new actual native captures of populated History and Settings must
show a clear visual improvement; light/dark normal/narrow layouts have no clipping;
long/Unicode transcripts and model/input names stay readable; keyboard/menus and
selection/search/copy/export work; preview still releases/yields; recording and
recovery never steal focus or warn on silence. Private lab only. Keep model,
storage, replacements, capture and platform support contracts unchanged.

Implementation: scoped GTK semantic CSS now supplies both native windows. Real
light/dark, high-contrast and enlarged-font captures are in
`CrossPlatform/evidence/visual-20261028/`. The default 500px narrow layout passes;
Sans 14 with full decorations has a roughly 553px natural minimum and was tested
at 560px. Live system theme/contrast notifications are implemented but changing
the desktop preference while a window is open remains manually unverified.

## Research and decisions

- [GNOME Search](https://developer.gnome.org/hig/patterns/nav/search.html): provide
  search for textual collections, keep it visible when central, show results as
  people type, and retain browsing. Koett places Search transcripts above recent
  entries, supports Ctrl/Cmd+F, and distinguishes no history from no matches.
- [KDE layout/navigation](https://develop.kde.org/hig/layout_and_nav/): group related
  controls by proximity, minimize navigation and adapt to narrow/tiled windows.
  Koett uses a transcript list and detail, collapsing to list/detail navigation
  when narrow. Settings use grouped rows, not a hierarchy of menus.
- [NN/g recognition](https://www.nngroup.com/articles/recognition-and-recall/): show
  choices and recent content rather than requiring remembered commands. Koett
  labels Record, Copy and Export, displays the configured shortcut, and provides
  controls outside its tray. Native icons supplement labels.
- [GNOME accessibility](https://developer.gnome.org/hig/guidelines/accessibility.html):
  preserve keyboard access, focus, contrast and accessible names. Respect system
  fonts, themes, scaling and reduced motion. Use native widgets and semantics.
- UI/UX Pro Max generated recommendations were reviewed critically: its marketplace
  layout, web font, gold accent and chatbot animation suggestions do not fit this
  native utility. Retain accessibility/performance guidance; keep Koett's coral
  accent, native typography and quiet surfaces.

## Window and workflow

Default window: approximately 960 by 680 logical pixels, resizable. A native
header contains Koett, readiness/recording state, Record/Stop, and Settings.
The primary workspace is History. Search stays above a 280–320px transcript list.
Rows show local date/time, a readable two-line text preview, and selection. The
detail pane contains the exact text, model/time metadata and labeled Copy/Export.
Avoid unrelated statistics, fake AI-generated titles, tutorial panels and decorative
cards. A narrow window shows one pane at a time with an obvious Back action.

First launch shows verified model progress, input selection and a clear recording
action. A failed permission or delivery must show a short actionable message and
available recovery. A missing optional portal/tray cannot make the app unusable.
Do not steal focus when a background dictation finishes. Opening history/settings
must not paste into those windows by accident; guard target focus or offer copy.

Settings is the one place for every Linux preference (2026-11-03, after Olle: "gather
all the settings to one place"). A sidebar lists pages in the Mac menu's order, most
used first: Dictation (microphone and live meter, recording mode, paste shortcut,
pause media), Model & formatting (speech model download/switch, Raw/S1-mini and its
controls; these apply right away), Draw on screen (Draw & Pin While Dictating, then its draw key, Include Selected Text in
Pins, the page-link setup command and the keyboard options, each nested and disabled
until its parent is on), Shortcuts, Words (dictionary and replacements),
and General (start at login, About). Use native sections and labeled controls; no JSON editor. Keep advanced model configuration out of the primary
workflow. Changes show validation near the control and persist predictably. The microphone
selection has one labeled live input-level meter for the selected option. Switching
options clears the old reading and previews the new device; unavailable inputs
show an error rather than silently substituting another microphone. Settings
preview never saves or transcribes audio, stops when closed, and yields to actual
dictation and releases capture when Audio is hidden. Linux paste uses one explicit shortcut
choice, shared across portal, compositor fallback and X11 delivery: Ctrl+V or
Ctrl+Shift+V. Terminal and GUI shortcuts differ, so the control must not imply
universal automatic target detection. Preserve focus guards and clipboard/history
recovery; never send multiple paste chords to guess what the target accepts.

## State and interaction

- Ready: Record is enabled and the active shortcut is visible.
- Recording: Stop is primary; show elapsed time and actual audio level when available. Since
  2026-11-02 the Linux pill is the timeline described in `docs/pill-timeline.md`; the text
  below describes the earlier Mac-matched pill, which macOS still uses. The earlier Linux
  recording pill followed the existing Mac source: compact dark 236x48 capsule,
  coral recording dot, 17 rounded rolling amplitude bars (coral center/white edges),
  divider and monospaced elapsed timer. Samples follow real microphone RMS using
  the Mac dB mapping/smoothing, never decorative synthetic motion. Reset on each
  capture; settle flat on silence. Toolkit reduced motion leaves a static
  waveform and functioning timer/status. Transcribing retains the capsule until
  delivery/cleanup; the surface never captures typing or pointer input.
- Transcribing: clear busy state without blocking history or closing the window.
- Done pill: a short result and one Copy button on the right. No other controls;
  the entire pill stays at most 48 px tall. Extra actions belong outside it.
- Saved result: refresh recent history without changing the user's current selection
  while they are reading an older entry. Copy confirms with Copied, then resets.
- Empty history: No transcripts yet and an enabled recording action.
- Empty search: No matches and an easy search clear action.
- Errors: identify the failed action and offer Retry/Copy where meaningful. Preserve text.

Use the native theme as semantic tokens: background, surface, text, secondary text,
border, selection, accent, error. One restrained coral accent for primary recording
and selected state; verify 4.5:1 text/3:1 controls in both system themes. Keep body
text at native readable size, transcript text around 16px, line length about 70ch,
and spacing on a consistent 4/8px rhythm. Avoid gradients and heavy shadows.

## Approved Settings and feedback pass — 2026-10-27

Use native GTK navigation for Audio, Shortcuts, Text and General. Open on Audio
with the selected input and its live meter visible. Keep Save outside the scrolling
content. Switching sections must retain unsaved edits; validation must reveal the
section containing the invalid control before focusing it. Audio preview must
release its capture when hidden, closed or saved and continue yielding to dictation.

Silence is normal, including long pauses to think, and must never produce a warning.
Only sustained excessive input levels produce a short, debounced amplitude warning.
Use measured input, reset between captures/devices, and avoid asserting that a
microphone disconnected or hardware clipped solely from amplitude. These warnings
must not stop recording or change transcription. Retain the existing Mac-style
waveform, timer, transcribing state and reduced-motion behavior.

An explicit delivery failure may show a temporary Copy transcript action near the
recording surface. Keep the exact completed text in the existing recovery cache
after the temporary action closes. Showing feedback must not request keyboard
focus; recording/transcribing remain pointer-transparent. A recovery action can
accept pointer input only while recovery is visible. A successful injected shortcut
does not establish that the target consumed the paste. Unsupported overlay sessions
must retain main-window recovery.

Acceptance: native light/dark and narrow Settings, keyboard navigation, invalid
replacement navigation, Save/reopen, preview release/resume, debounced warning
signals, silent thinking pauses and recovery-copy cleanup in a private lab. Preserve model choice, settings
values until explicit Save, history durability, and independent delivery/storage
errors. No model picker or automatic personal-replacement learning in this pass.

## Native menus and navigation — 2026-10-28

Tray activation opens History. Recording is an explicit Record/Stop action;
Starting and Transcribing disable capture, while actual engine failures show
Retry. Internal control messages and shortcut notices never replace the phase.
The tray and the header Menu carry quick actions only: Record, Ask About Screen,
Transcribe media, History, Copy/Paste last transcript, Recovery, Settings… and
Quit. No preference lives in a menu; `koett menu-*` commands remain the agent
surface for the same values, and `menu-open models|formatting` opens Settings on
the Model & formatting page.

The visible Settings button remains in the header beside a compact native Menu.
Copy last stays disabled until a completed result exists. Ctrl+, opens or reuses
Settings, Ctrl+F focuses search and Ctrl+Q quits. Hiding the window leaves
dictation available through the shortcut and tray. Before/after captures:
`CrossPlatform/evidence/settings-unified-20261103/`.

## Shared history contract

Canonical SQL: `Sources/Koett/Resources/History/schema.sql`. Rust includes that
file at compile time; Swift bundles it. Schema version 2 uses `transcripts.id`
(stable text ID), `created_at_ms` (UTC epoch milliseconds), `model`, `text`,
optional `raw_text`, external-content FTS5 and chronological paging by
`(created_at_ms, id)` descending. Identical text in different recordings remains
distinct. An existing ID with different content is an error, never a silent drop.

Source/version ownership reconciles older imported search rows only after a full
verified import. Native dictations and raw legacy fragments remain protected.
The SQLite-only v1 upgrade protects all existing rows because their ownership is
unknown; duplicates from that development schema may remain. Legacy import runs
once in the background at startup, never from page/get queries.

Rust public storage surface for the Linux UI:
`history::HistoryStore::open(path)`, `HistoryStore::open_default()`,
`save(&HistoryRecord)`, `page(query, before: Option<&HistoryCursor>, limit)`,
`get(id)` and `import_legacy(path)`. `HistoryRecord` has id, created_at_ms,
model, text and raw_text. `HistorySummary` has id, created_at_ms, model and preview.
`HistoryCursor` has created_at_ms and id; `page` returns up to 50 summaries in
descending chronological order. Search treats words as literal terms, not SQL/FTS
operators. Keep full text out of the initial list query.

Preserve existing `append_transcript_resilient`/last-result recovery APIs while
cutting their internals over to SQLite. Store/query/import off UI and audio threads.
Private files, bounded queries and durable failed-save fallbacks remain required.

## Pill and overlay UI rules

These rules come from the Mac pill that passed its live test on 2026-11-02
(`e0ca1b9`, PRs #9, #11, #19, #21). Both platforms follow them. Change a rule
only with a new live test, and record why in `docs/decisions.md`.

### Shape

- **One row, always.** Every state is a 300×48 capsule (radius 23). Nothing ever
  grows taller. Hover actions, results, Not pasted and errors all fit inside
  that row. A taller pill read as "goes big" and was rejected.
- **Fixed width from the first frame.** The level strip spans its full width
  (64 columns, 3 px pitch, 2 px bars) on the very first sample; time before the
  start draws as dim silence dots. The pill never gets longer as you talk.
- **The strip is a rolling window, not a whole-dictation overview.** Mac: the
  last 9.6 s, as 64 columns of 150 ms. The newest audio enters at the right
  edge; columns slide left by elapsed time, with sub-column precision, and fade
  out over the left six columns. Nothing is ever re-binned, so 0:05 and 20:00
  look the same, and memory stays at 66 columns.
- **Fixed slots in the row:** glyph centered at x = 23; strip from x = 44 to
  236; clock right-aligned in 238–284. Hover buttons start at x = 40 and must
  end before the clock (x < 238).
- **Light and dark follow the system.** Use the shared palette
  (`PillPalette` / Linux `DARK`/`LIGHT`); coral is the only accent.

### States

| State | Row content | Leaves |
| --- | --- | --- |
| Recording | coral dot (pen while drawing, two bars while paused), strip, `m:ss` | on stop |
| Hover while recording or transcribing | Cancel, Pause/Resume, History replace the strip; the clock stays | when the pointer leaves |
| Transcribing | spinner, frozen dimmed strip, `NN%` only when the engine reports real progress | on result or error |
| Done | 5-bar coral mark, one-line preview, **Copy** on the right | 1.75 s, held while hovered |
| Not pasted | warning glyph, `Not pasted · saved`, **Paste** and **Copy** on the right | only when acted on or clicked |
| Error with text or audio to rescue | warning glyph, message | only when clicked or replaced |
| Other errors | warning glyph, message | 5 s |
| Media result | same row as Done | 10 s, held while hovered |

### Motion and timing

- **Morph in place.** Transcribing becomes Done (or Not pasted) without moving or
  resizing: a 0.24 s ease-out (cubic) crossfade. The old content fades faster
  (`1 − 1.6 t`), the new content fades in and slides 8 px from the right, and
  Copy fades in with it.
- **Leave quickly.** Done stays 1.75 s, then the panel fades out over 0.25 s.
  Hovering holds it; once the pointer leaves, it fades after 0.8 s. Copy shows
  "Copied" and fades after 1 s. Nothing else extends it. The old 10 s timeout
  was rejected as "lingers a long time after I'm finished".
- **Things that need rescuing never time out.** Not pasted, and errors that hold
  text or audio, stay until they are used or clicked.
- **Reduce Motion:** keep sampling the meter at 30 Hz, redraw the strip once a
  second, stop the spinner, and skip the morph and the fade.
- **The pill never takes focus.** It is a non-activating panel that cannot
  become key or main. It takes the pointer only in states that have buttons.

### How a pill change is verified

1. Render every state off-screen in dark and light with
   `KOETT_PILL_SNAPSHOT_DIR=/tmp/pill swift test --filter PillSnapshotTests` (Mac),
   so the user's screen is never driven. Include recording at **0:05, 1:00, 5:00 and
   20:00**, hover, paused, drawing, transcribing, Done, Not pasted and error.
2. Render the morph at t = 0, 0.25, 0.5, 0.75 and 1, and look at the frames
   stacked. A frame where two glyphs or texts fight is a bug.
3. Look at every PNG before claiming done; keep them in a private cache outside the tracked tree and in
   the PR body.
4. Unit tests guard the rules: every state is 48 px, its buttons fit inside
   without overlapping the clock, the strip reaches both edges at every point
   over 20 minutes, a peak moves only by elapsed time, Done leaves after about
   2 s and is held by hover, and Not pasted and rescue errors stay.
5. Install and let the user try it. Only a live try counts as passing.

### Rejected, and why

- **Whole-dictation timeline that halves its resolution when full.** The strip
  visibly grew and then jumped ("once it reaches a threshold, it compresses").
- **Smooth resampling of the whole dictation.** It still squeezes, and at
  20 minutes each bar covers about 19 s and looks like a flat block. A rolling
  window has no compression to hide.
- **A 300×88 pill with an action row on top**, for Done, Not pasted or hover. It
  "goes big".
- **A Done receipt with the strip, `Done · m:ss` and History.** It "looks weird";
  the one-row Copy result was wanted back.
- **A 10 s Done timeout.**
- **A decode playhead on the strip.** FluidAudio's progress counts dispatched
  chunks of trimmed audio, so a playhead on wall time misleads. Show a
  percentage, or only the spinner.
- **Model name in the hover row.** There is no room for it at 48 px.

### What helped

- The user's literal complaints, quoted and used as the spec. Every rule above
  answers one of them.
- The `verification-before-completion` skill: a claim needs matching evidence,
  so every state was rendered and looked at, and the user's live try was the
  final gate.
- The `development` skill: check library behavior against its source before
  building on it (the FluidAudio progress stream), and make test fixtures prove
  the bug they guard against.
- No design or animation skill was loaded. The timings came from keeping
  motion short and in place, then checking the stacked morph frames.

### Linux status (2026-11-03)

Linux now matches the Mac pill (`344d11f`). Its strip is the same rolling window
(`CrossPlatform/src/linux/timeline.rs`): the last 9.6 s as 64 columns of 150 ms,
sampled at 30 Hz so it slides left at a constant rate, silence included. It
never re-bins, and it fades over the left six columns. Not pasted shows
**Paste** and **Copy**: Paste closes the pill and pastes the last transcript,
and Copy closes it after 1 s. Transcribing shows a frozen dimmed strip, `NN%`
from the engine's real VAD-segment plan, and Cancel/History on hover. Leaving a
held pill closes it after 0.8 s. Remaining platform differences: Linux has no
"other errors" pill state (engine errors show in the main window), and Esc only
cancels on X11 (Wayland uses the hover Cancel button or `koett cancel`). Linux
renders: `CrossPlatform/evidence/pill-rolling-20261103/`.

## Usability verification

Exercise first launch, choose input, record/stop, recover failed paste, find an old
phrase, copy/export, change shortcut and reopen settings. Check each with keyboard
and pointer. Capture actual native light/dark, narrow/large, empty/populated/error
states. Measure opening/searching a generated 100,000-entry archive while capture
continues. Visual mockups are design references, not native runtime evidence.
