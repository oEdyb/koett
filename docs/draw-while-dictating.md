# Draw while dictating

Olle asked on 2026-11-02 for Scrawl's sketch-and-talk behaviour in Koett. Scrawl
(Kit Langton, MIT, github.com/kitlangton/scrawl, v0.3.14 inspected on Olle's Mac)
is the behaviour reference only; no Scrawl code, assets, icons or name are used.

## Behaviour (Linux, implemented)

- Settings › Text: **Draw on screen while dictating**, off by default, and
  **Draw key**: Shift (default), Ctrl or Alt.
- While a dictation records, a transparent overlay covers every monitor. Hold
  the draw key and move the pointer to leave red (#ff3b30) ink. Without the key,
  clicks pass through to the apps underneath. Ctrl+Z removes the last stroke of
  a sketch that has not been captured yet.
- Strokes on one monitor form one sketch. A sketch closes 0.7 s after the key is
  released (or at 12 strokes / 20 s, or when recording stops); that monitor is
  captured with the ink visible, then the ink fades 1.5 s after the pen lifted.
  Captures are serialized per monitor: while one is pending, only that sketch's
  ink is shown there (other and newer ink reappears after it), so an image never
  contains another sketch's strokes. The screenshot starts only once GDK reports
  wp_presentation feedback for a frame painted that way (a discarded frame is
  repainted; 2 s fallback for compositors without the protocol).
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

- The overlay takes exclusive keyboard focus while recording (Wayland exposes no
  global key state). Typing into another app during a recording is blocked while
  the feature is on (see the proposal below); compositor and portal shortcuts
  still work. Focus returns
  to the previous window when recording stops, before paste.
- The ink (and the recording pill when the ink is near it) is part of the
  captured image.
- Multi-monitor: one overlay and one capture per monitor; portal images that
  span the desktop are cropped per monitor (unit-tested). The lab has a single
  virtual output, so multi-monitor drawing is not yet verified on hardware.
- Pins (Scrawl's Shift-click), click keyframes and spoken cue keyframes are not
  implemented.
- `wl-copy` cannot report when the target read the image, so the wlroots path
  waits a fixed 700 ms before switching the clipboard to text. The portal path
  waits for the actual read (max 1.5 s) plus 160 ms.
- Nemotron streaming and merged long chunks fall back to evenly estimated word
  times, so linking there is approximate.

## Mac to-do (not built; requires the MacBook)

1. Settings toggle and draw-key picker (Shift default) in the SwiftUI settings.
2. Overlay: one borderless, transparent, non-activating `NSPanel` per `NSScreen`
   at `.screenSaver` level, `ignoresMouseEvents = true` except while the draw key
   is held. Detect the key with an `NSEvent` global + local monitor for
   `.flagsChanged` (needs Accessibility, which Koett already has) — no focus
   stealing is needed on macOS.
3. Capture: ScreenCaptureKit `SCScreenshotManager.captureImage` for the display
   under the sketch (Screen Recording permission; prompt on first enable).
4. Word timestamps: FluidAudio/Parakeet token timings into the transcript model;
   port `annotation.rs` (`keyframe_word_ranges`, `keyframe_summary`) and its tests
   to Swift so both platforms produce identical bullets.
5. Contact sheet with Core Graphics using the same layout constants.
6. Paste: `NSPasteboard` PNG + Command-V, wait for the read, then the text.
7. Run the root AGENTS.md Swift checks and a live test with Olle.

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

Recommendation: make **B the default** and keep **A as an opt-in** ("Shift
anywhere — blocks typing while recording"). On niri, bind one key to
`koett draw`; draw mode starts focused, Shift-move draws as now, and focus
returns on release or Esc. On KDE, register a portal hold shortcut. Offer C
only as a documented advanced setting, never by default. B needs one small
control command and a per-desktop setup note, and removes the typing block
without new privileges.
