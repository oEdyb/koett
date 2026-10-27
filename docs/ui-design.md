# Koett desktop UI

Approved scope: Linux port and UI first, searchable history on Linux and macOS.
Olle explicitly found the previous interface unintuitive. This document governs
the implementation; visual polish must support observable tasks.

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

Settings: Input (microphone), Recording (mode and shortcut), Text (replacement
pairs), General (launch at login, pause media). Use native groups and labeled
controls; no JSON editor. Keep advanced model configuration out of the primary
workflow. Changes show validation near the control and persist predictably. The microphone
selection has one labeled live input-level meter for the selected option. Switching
options clears the old reading and previews the new device; unavailable inputs
show an error rather than silently substituting another microphone. Settings
preview never saves or transcribes audio, stops when closed, and yields to actual
dictation. A broader visual redesign remains deferred.

## State and interaction

- Ready: Record is enabled and the active shortcut is visible.
- Recording: Stop is primary; show elapsed time and actual audio level when available.
- Transcribing: clear busy state without blocking history or closing the window.
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

## Usability verification

Exercise first launch, choose input, record/stop, recover failed paste, find an old
phrase, copy/export, change shortcut and reopen settings. Check each with keyboard
and pointer. Capture actual native light/dark, narrow/large, empty/populated/error
states. Measure opening/searching a generated 100,000-entry archive while capture
continues. Visual mockups are design references, not native runtime evidence.
