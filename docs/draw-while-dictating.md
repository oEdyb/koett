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
- Recognizer word timestamps link speech to sketches: each sketch owns the words
  from the start of the clause in progress when it began until the next sketch;
  words before the first sketch belong to it. Clauses break at `, . ; : ? ! …`
  or a 1.5 s pause.
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
  own drawing cue from these events. An undone sketch's band stays on the pill
  (the registry only upserts).

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
  the feature is on; compositor and portal shortcuts still work. Focus returns
  to the previous window when recording stops, before paste.
- The recording pill and the ink are part of the captured image; keyframes are
  whole monitors (no auto-crop yet).
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
