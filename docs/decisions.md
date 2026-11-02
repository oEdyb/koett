# Koett decisions

## 2026-10-26: integrate the existing native port with current Mac recovery

The Linux UI/history work branches from `origin/codex/overnight-2026-10-03`
at b6e2c73, which contains the current Mac microphone/recovery fixes. Integrate
the existing Rust port from `origin/feat/windows-linux-v0.1` at e85d0d0.
Keep Mac Sources/Tests from the newer branch during conflict resolution;
optional multilingual model changes from the draft are outside this task.
Do not merge or update the public draft PR as part of this local integration.

SQLite/FTS5 replaces the active growing Markdown archive on Linux and macOS.
The user confirmed in-app recent entries, search, copy and export, preserving
original legacy files. GTK4 supplies Linux windows; SwiftUI/AppKit supplies Mac
windows. This adds no browser runtime to dictation. See plan.md for sequencing.

## 2026-10-26: verified source versions own imported search rows

Schema v2 tracks each legacy path's verified content version and its imported IDs.
Re-importing an appended archive replaces obsolete imported search rows atomically,
while native recordings, other active source references and original bytes remain
protected. Earlier development schemas lack provenance, so their existing rows
are protected during upgrade rather than guessed away. History opens and queries
never scan the legacy archive; startup imports run in the background.

Linux GUI verification uses the private `agent-lab koett` compositor, input,
audio and portals. Host desktop checks require explicit authorization. This is a
test environment choice; the product still uses native GTK/Swift windows.

## 2026-10-29: scope the native Linux visual theme

History and Settings share a small GTK CSS provider with raw palette values mapped
to semantic roles. Scope it to Koett windows, retain native controls/fonts and
high-contrast colors, and keep the recording pill's established appearance. This
provides coherent surfaces without adding a UI framework or changing dictation.
The reference brief and inspected native captures live in docs/ui-design.md.

## 2026-11-02: the recording pill becomes a dictation timeline

Olle picked concept C from `docs/pill-concepts.html`. On Linux the pill is now
300×48 and shows the whole dictation as one strip of measured levels. Quiet
columns are dots, and sketches are numbered bands. The strip halves its
resolution each time it fills, so ten minutes still fits. While decoding, a
playhead reports finished/planned VAD segments, and the plan is published only
once known. Hover adds Cancel, Pause and History. After the dictation ends, the
pill shows Done (Copy, History) or Not pasted (Paste again, Copy, History).
The action row sits above the strip, so the bottom-anchored strip never moves.

The pill now takes pointer input while recording, so hover works, but it never
takes keyboard focus; decoding stays pointer-transparent. Pause discards input
at the capture callback, and the 10-minute limit still counts wall time.
Cancel drops the capture with no transcript, paste or history row.

Esc cancels only on X11, where the grab exists only while recording. Wayland has
no focus-free way to hear a bare Esc: a portal binding would take Esc from every
app permanently, and layer-shell keyboard focus would steal focus. Wayland
therefore offers the pill's Cancel button and `koett cancel`, which can be bound
in the compositor. Silence dots come from the level floor, not from Silero, since
the live path only runs VAD after 20 s. Draw mode feeds sketch bands through
`koett_engine::sketch`; see `docs/pill-timeline.md`.

## 2026-11-02: dictionary uses decoder hotwords, kept only where they spell a term

Three approaches were built in parallel and scored on one synthetic benchmark
(`Benchmarks/Dictionary/`): sherpa-onnx hotwords, fuzzy text correction, and
suggestions learned from corrections. Hotwords gave the large gain (31% → 73%
term recall on Parakeet v2) but a single biased stream recapitalized ordinary
words. Koett therefore decodes plain and biased streams in one batch and adopts
only biased spans that spell a dictionary term: 73% recall, 0 false terms, 0
broken near-miss sentences, about twice the decode time while a dictionary exists.
One-edit fuzzy matching was dropped: +2 points recall for 3 false rewrites
("parakeets" → "Parakeet"). Exact spelling normalization and user sounds-like
forms stay. An empty dictionary keeps the old greedy path. Details and the Mac
plan: `docs/dictionary.md`.

## 2026-11-02: draw while dictating uses layer-shell, screen stills and one contact sheet

Olle asked for Scrawl's sketch-and-talk behaviour and chose its paste format: one
numbered contact-sheet PNG, then `- #N (M:SS) “words”` text. Linux uses one
layer-shell overlay per monitor during dictation (off by default). Wayland has no
global key state, so the overlay holds exclusive keyboard focus while recording
to see the draw key, and releases it before paste. KWin reported stale modifier
state on pointer events, so drawing follows the overlay's own key events.
Stills, not video: each closed sketch captures its monitor with the ink visible,
first through `grim` (wlr-screencopy: niri, Sway, Hyprland; Olle's niri disables
the Screenshot portal) and otherwise through the non-interactive Screenshot portal
(KDE asks once). Parakeet token timestamps link words to sketches. Image and text
are pasted sequentially, because a single clipboard offer with both types makes
each app choose only one of them. GNOME and X11 lack layer-shell and keep normal
dictation. Details, limits and the Mac to-do: docs/draw-while-dictating.md.

## 2026-11-02: bare Shift via keyboard state, one hold per sketch

Olle chose maximum UX over the compositor-shortcut option: bare Shift, each
hold one separate sketch (release ends it), many sketches per dictation, and
typing never blocked. Wayland clients cannot see Shift without focus, so the
opt-in "Keep typing" mode samples held keys with EVIOCGKEY while a drawing
dictation records and uses only the modifier bits (the kernel has no
modifier-only query; samples are wiped, nothing is stored, key events are never
read). Access comes from a one-time setup Settings copies: a `koett-keyboard`
group with a read-only udev ACL on keyboards (review preferred this over
`uaccess`, which grants read and write). Without access, the focused overlay
remains the fallback and Koett says so. Keyboards are rescanned during a
recording for hot-plug. Read access to keyboards for the user's apps is the
cost, which is why it is opt-in, not default.
