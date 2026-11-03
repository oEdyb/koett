# Draw while dictating

Olle asked on 2026-11-02 for Scrawl's sketch-and-talk behaviour in Koett. Scrawl
(Kit Langton, MIT, github.com/kitlangton/scrawl, v0.3.14 inspected on Olle's Mac)
is the behaviour reference only; no Scrawl code, assets, icons or name are used.

## Behaviour (Linux, implemented)

- Settings › Text: **Draw on screen while dictating**, off by default, and
  **Draw key**: Shift (default), Ctrl or Alt.
- A recording never blocks typing by default. Koett reads the draw key from the
  keyboard devices whenever they're readable, so the overlay never takes
  keyboard focus. About 60 times a second it samples which keys are held
  (`EVIOCGKEY`; Linux has no modifier-only query, so a sample briefly contains
  every held key), uses only Shift/Ctrl/Alt/Super, stores nothing and wipes the
  sample with volatile writes. It never reads key events or typed text, and
  opens keyboards only while a dictation with drawing records. Keyboards are
  found through sysfs capabilities and rescanned every 2 s, so hot-plugged ones
  join and unplugged handles are dropped; unreadable ones are reported.
- A draw-key hold under 120 ms does nothing, so typing a capital letter neither
  blocks clicks nor flashes the crosshair, and ink under 12 px is dropped as a
  resting hand rather than becoming a sketch.
- If keyboards can't be read, drawing stays off for that dictation with a
  one-line hint (main window, plus one desktop notification per run). Only the
  opt-in "If keys can't be read, let drawing take the keyboard" uses the focused
  overlay, which pauses typing while recording; Ctrl+Z undo needs that mode.
- Setup (Settings copies it): a `koett-keyboard` group, the user added to it,
  and a udev rule that adds a **read-only** ACL for that group on keyboard
  devices (owner and the `input` group unchanged). Log out and back in once.
  Any app the user runs can then read keyboards, which is why this is opt-in.
- While a dictation records, a transparent overlay covers every monitor. Hold
  the draw key and move the pointer to leave red (#ff3b30) ink. Without the key,
  clicks pass through to the apps underneath. Ctrl+Z removes the last stroke of
  a sketch that has not been captured yet.
- One draw-key hold is one sketch, as in Scrawl: releasing the key closes it
  and the next hold always starts a new sketch, on the same monitor or another,
  so a long dictation can mix talking and many separate sketches. Moving to
  another monitor during one hold starts a sketch there. A closed sketch's
  monitor is captured with the ink visible; the ink fades 1.5 s after release.
  Captures are serialized per monitor: while one is pending, only that sketch's
  ink is shown there (other and newer ink reappears after it), so an image never
  contains another sketch's strokes. The screenshot starts only once GDK reports
  wp_presentation feedback for a frame painted that way (a discarded frame is
  repainted; 2 s fallback when no timing arrives). This relies on GDK's frame
  timings: KWin, niri, Sway and Hyprland implement wp_presentation, so the time
  is the compositor's. On a compositor without it GDK may report an estimated
  presentation time, which GTK does not let Koett tell apart, so there the
  guarantee is only "GDK considers the frame done".
- Sketch numbers follow creation order everywhere (sheet, bullets, pill), even
  when sketches made during a pause share an audio time. Captures that finish or
  fail after Stop still renumber the pill until the next dictation starts.
- Sketch times for speech linking use the captured-audio clock: paused spans are
  excluded, matching the transcript's word times. The pill's bands use its own
  wall clock (`koett_engine::sketch::elapsed_ms`), which includes pauses.
- Frames are scoped to one recording. Every recording start, with drawing on or
  off, starts a new scope; a cancelled or failed recording discards its frames,
  and captures finishing after their recording ended are rejected.
- Recognizer word timestamps link speech to sketches: each sketch owns the words
  from the start of the clause in progress when it began until the next sketch;
  words before the first sketch belong to it. Clauses break at `, . ; : ? ! …`
  or a 1.5 s pause. When replacements or cleanup change the text, recognizer
  words are aligned to the final words (longest common subsequence), so removed
  fillers cannot shift a sketch's words; unmatched spans map proportionally.
- Each captured monitor is cropped around the sketch's ink, keeping at least
  45% of the monitor in each direction as context.
- On paste: one numbered contact-sheet PNG (sketches in time order, number chip
  per tile, 1600 px long edge) is pasted with Ctrl+V; once the target has read it,
  the text is pasted with the configured chord as
  `- #N (M:SS) “words”` bullets (`- #N (M:SS) marked` when nothing was said).
  Without sketches dictation is unchanged.
- History stores the bullet text, with the plain transcript as raw text. Images
  stay in memory only, like Ask's screen image; nothing is written to disk
  except the portal's own screenshot file, which Koett deletes immediately.
- Ask recordings never show the overlay.
- Every sketch change is published to `crate::sketch` as
  `{index, start_ms, end_ms, live}` on the pill's dictation clock, numbered like
  the pasted sheet and text; the Timeline pill draws the numbered bands and its
  own drawing cue from these events. Undo or a failed capture republishes the
  renumbered sketches and sends `Removed { index }` for the freed number.

## Desktop support and limits

| Desktop | Overlay | Capture | Status |
|---|---|---|---|
| KDE Plasma 6 (Wayland) | layer-shell | Screenshot portal (one-time "Allow apps to take screenshots") | Verified end to end in private KWin 6.8.1 lab |
| niri | layer-shell | `grim -o OUTPUT` (wlr-screencopy) | Same code paths; not run on Olle's visible desktop (needs authorization) |
| Sway, Hyprland, other wlroots | layer-shell | grim, else portal | Unverified |
| GNOME Wayland | no layer-shell | — | Unavailable; dictation unaffected |
| X11 | no layer-shell | — | Unavailable; dictation unaffected |

- The ink (and the recording pill when the ink is near it) is part of the
  captured image.
- Multi-monitor: one overlay and one capture per monitor; portal images that
  span the desktop are cropped per monitor (unit-tested). The lab has a single
  virtual output, so multi-monitor drawing is not yet verified on hardware.
- On Linux, pins (Scrawl's Shift-click), click keyframes and spoken cue keyframes are not
  implemented.
- `wl-copy` cannot report when the target read the image, so the wlroots path
  waits a fixed 700 ms before switching the clipboard to text. The portal path
  waits for the actual read (max 1.5 s) plus 160 ms.
- Nemotron streaming and merged long chunks fall back to evenly estimated word
  times, so linking there is approximate.

## Mac (built 2026-11-02, live check pending)

Same output as Linux: one numbered contact sheet, then the same
`- #N (M:SS) “words”` bullets. Files: `SketchLinking.swift` (port of
`annotation.rs`), `SketchBoard.swift` (holds, numbering, capture phases),
`SketchOverlay.swift` (panels, key, pointer), `SketchCapture.swift`,
`SketchSheet.swift`, `SketchFrames.swift`, `SketchPaste.swift`,
`SketchDictation.swift`, `DrawKey.swift`.

- Menu › **Draw on Screen**: **Draw & Pin While Dictating** (off by default),
  **Include Selected Text in Pins** (on by default) and
  **Draw Key** Shift (default), Control or Option. A key the modifier-only
  dictation shortcut uses is disabled (Option with the default "Either
  Option"), and Koett falls back to the first free key. Turning drawing on
  asks for Screen Recording once.
- One transparent, non-activating panel per screen at `.screenSaver` level,
  shown only while a dictation records. The draw key comes from `.flagsChanged`
  global and local monitors (modifier flags only, no key events), so the
  panels never take keyboard focus and typing keeps working. The key draws
  only when it is the only modifier down, so Command-Shift-4 or Control-Shift-T
  never draw. While it is held the panels take the pointer; otherwise clicks
  pass through. The pointer is sampled at 120 Hz while held.
- A hold starts a sketch only after the pointer moves 3 pt, so Shift for a
  capital letter leaves no dot. The sketch's start is the sample where the
  movement began. Otherwise holds, screens and numbering follow Linux.
- Capture: on release, ScreenCaptureKit takes a still of that display with
  every Koett window excluded (overlay, pill, panels), and Koett draws only that
  sketch's strokes onto it before cropping (same crop rule as Linux). A capture
  therefore cannot contain another sketch's ink or the pill, and needs no
  frame-presentation sync. Ctrl+Z undo is not ported: the overlay never has
  keyboard focus and captures start on release.
- Pins (Mac only so far): a click while the draw key is held drops numbered
  pin #N at the click instead of ink. It shares numbering, linking, the sheet
  and the paste with sketches; its tile is a crop around a red ring, and its
  bullet gains a context line read through Accessibility only (window owner,
  page URL and title, link or image under the pointer, YouTube `t=` from the
  `ytp-progress-bar` slider, selected text capped at 500 characters). Shift+move
  never pins, Shift+click never starts ink; the full gesture table and its tests
  are in `screen-context-proposal.md` and `SketchPinTests.swift`.
- Clocks: linking uses the recorder's written-audio time (`currentTime`, paused
  buffers are never written); the pill uses its own wall clock. Parakeet token
  times come from FluidAudio `tokenTimings` (`▁` marks a word start); they are
  already mapped back across silence trimming. Nemotron and mismatched tokens
  fall back to an even estimate over the recording.
- Scope: every dictation start calls `SketchFrames.begin`; Cancel discards the
  overlay and frames; captures from another dictation are rejected. Delivery
  waits up to 8 s for open captures, then the last Cancel check runs.
- Pill: `SketchUpdate.upsert`/`.removed(index:)` go through
  `RecordingOverlayController.applySketch(_:session:)`, accepted after Stop for
  the same dictation until the next one starts, so a capture failing during
  transcription renumbers the bands.
- Paste: the sheet is offered lazily as PNG and TIFF, Command-V is posted,
  Koett waits for the target's read (max 1.5 s) plus 160 ms, then pastes the text
  as before. History stores the bullets with the plain transcript as raw text.
  A clipboard manager that reads first makes Koett wait the full 1.5 s.
- Proof: `SketchLinkingTests.testOutputMatchesLinuxByteForByte` replays 300
  dictations generated by the Linux linker (`scripts/sketch-linking-golden.rs`);
  a 0.1 s change to the clause pause fails 7 of them. Sample sheet render:
  `docs/evidence/mac-draw-20261102/contact-sheet.png`.
- Terminals: when the frontmost app is Terminal, iTerm2, Ghostty, kitty,
  WezTerm, Alacritty or Warp (by bundle ID), the sheet is pasted with
  Control-V so terminal agents such as Claude Code read the image; the text
  still uses Command-V. VS Code and Cursor are not detected (their terminal
  shares the editor's app and its focus is not readable without forcing
  Electron accessibility on). Draw on Screen › **Paste Sketch Image with
  Control-V in <app>** flips the chord for the app in front, stored per bundle
  ID in `sketchImagePasteControlV`. In a plain shell, Control-V is quoted
  insert, so the first pasted character arrives literally.
- Limits: screens added during a recording get no panel until the next one.

## How this was chosen (2026-11-02)

Olle asked for parallel takes. Two sub-agents built alternatives from the same
base commit; each was compared on its own checks and against the lab flow above.

- **Speech linking take** (`take/sketch-linking`): a 16-scenario labelled corpus
  (draw-then-speak, two sketches in one clause, long pauses, filler removal…).
  The original proportional mapping scored 13/16 scenarios, 48/51 words; the
  word-alignment mapping scored 16/16, 51/51. Adopted, with the corpus as
  regression tests (`src/annotation/evaluation.rs`).
- **Clean-frame capture take** (`take/sketch-clean-frame`): capture a clean frame
  when a sketch starts, composite the ink in software, crop around it. Its crop
  was adopted on top of the verified capture-with-ink path. Clean-frame capture
  was not: it was never measured on a real capture, can race the first stroke,
  and the first frame can still contain the pill.

## Proposal: drawing without blocking typing (for Olle, 2026-11-02)

Today the overlay holds exclusive keyboard focus for the whole recording,
because a Wayland client cannot see a bare Shift press unless it has focus.
That blocks typing into other apps while dictating with drawing on. Options:

| Option | How the draw key is seen | niri | KDE | Cost |
|---|---|---|---|---|
| A. Current | Overlay keeps keyboard focus all recording | Works | Works | Typing blocked while recording |
| B. Compositor trigger | A desktop shortcut starts draw mode; only then does the overlay take focus. It sees the key release itself and gives focus back | Press-to-start: niri binds fire on press (no release action, to our knowledge), e.g. `Mod+Shift+D { spawn "koett" "draw"; }`; draw mode ends when Shift is released or with Esc | The GlobalShortcuts portal reports press **and** release, so a true hold works; KDE does not allow a bare Shift binding, so it is a chord (e.g. Ctrl+Alt+D) | Not bare Shift: one extra key or chord per sketch; typing never blocked |
| C. Read keyboard devices (evdev) | Koett reads `/dev/input` key state directly | Works, true bare-Shift hold | Works | Needs the `input` group or a udev rule, which would let Koett read every keystroke system-wide; plan.md rules this out as a default |

**Decision (Olle, 2026-11-02): maximum UX — bare Shift, many separate sketches
per dictation, typing never blocked. Built as C (device state, opt-in with
guided setup) with A as the fallback.** Original recommendation, kept for the
record: make **B the default** and keep **A as an opt-in** ("Shift
anywhere — blocks typing while recording"). On niri, bind one key to
`koett draw`; draw mode starts focused, Shift-move draws as now, and focus
returns on release or Esc. On KDE, register a portal hold shortcut. Offer C
only as a documented advanced setting, never by default. B needs one small
control command and a per-desktop setup note, and removes the typing block
without new privileges.
