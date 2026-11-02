# Recording pill: timeline

Concept C from `docs/pill-concepts.html`, built on Linux on 2026-11-02. Evidence
lives in `CrossPlatform/evidence/pill-timeline-20261102/`.

## States (Linux)

| State | Size | Shows | Actions |
| --- | --- | --- | --- |
| Hidden | none | Nothing; idle has no on-screen nub | none |
| Recording | 300×48 | Coral dot, whole-session strip, `m:ss` | Hover adds a row: Cancel, Pause/Resume, model name, History |
| Drawing | 300×48 | Coral 2 px ring, pen glyph, live band | As recording |
| Paused | 300×48 | Two-bar pause glyph; strip adds silence | Hover: Resume |
| Transcribing | 300×48 | Spinner, playhead at decoded/planned, `3/5` | None (pointer-transparent) |
| Done | 300×88 | Check, final strip, `Done · m:ss` | Copy, History; stays 10 s, longer while hovered |
| Not pasted | 300×88 | Warning glyph, `Not pasted · text saved` | Paste again, Copy, History |

The strip has 64 columns of 2 px bars with 1 px gaps. Each column covers 250 ms
and the strip halves its resolution each time it fills. Columns under the
silence threshold draw as dots. Each sketch band spans start→end; a live band
is outlined and fills when finished. Reduced motion stops the spinner and
redraws the strip once per second.

## Sketch-event interface

`CrossPlatform/src/sketch.rs`, exported as `koett_engine::sketch`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SketchEvent {
    pub index: u32,     // 1-based; the number pasted with the sketch
    pub start_ms: u64,  // ms since the dictation started, when drawing began
    pub end_ms: u64,    // latest drawing time; keep raising it while live
    pub live: bool,     // true while Shift is held
}

pub fn elapsed_ms() -> Option<u64>;            // the pill's clock; None when idle
pub fn publish(event: SketchEvent) -> bool;    // call from any thread
pub fn drain() -> Vec<SketchEvent>;            // the pill calls this
pub fn set_recording_start(Option<Instant>);   // the pill calls this
```

Rules:
- An event replaces any earlier event with the same `index` (upsert).
  Republish a live sketch as it grows, then send it once more with
  `live: false`.
- Take `start_ms`/`end_ms` from `elapsed_ms()` so they share the pill's clock.
  Paused time is included, because the strip shows wall time.
- `publish` returns false and drops the event when no dictation is active,
  when `index == 0`, or when `end_ms < start_ms`. Pending events are cleared
  when a new dictation starts.
- The pill keeps at most 256 distinct sketches per dictation. New indices
  beyond that are ignored; known indices keep updating.
- Undo or a failed capture sends `Removed { index }` (Swift:
  `SketchUpdate.removed(index:)`) for the freed number after republishing the
  renumbered sketches.

Debug builds also accept `koett 'sketch-debug INDEX START END|now live|done'`.
The lab uses it to draw bands before draw mode exists.

## Decode progress

`koett_engine::transcription::decode_progress() -> (decoded, planned)`.
`planned` stays 0 until the final count is known:
- Long Parakeet dictations: segments decoded during recording plus the
  segments still pending at stop.
- Short Parakeet dictations: 1.
- Nemotron: never set, so the pill shows only the spinner.

## Mac (Swift)

Built on 2026-11-02 in `Sources/Koett/PillTimeline.swift`, `PillView.swift`,
`PillButton.swift`, `PillPalette.swift`, `RecordingOverlay.swift` and
`EscapeKeyMonitor.swift`. It matches the Linux states above, with these
Mac-specific choices:

- Since 2026-11-02 evening, after a live test where the strip visibly grew
  and then compressed, the Mac strip is no longer a whole-dictation timeline. `PillTimeline` is a fixed 64-column
  window of 150 ms columns (9.6 s). Time before the start draws as silence
  dots, so the strip is full width from the first frame. Columns slide left by
  elapsed time with sub-column precision and fade out over the left six
  columns; nothing is ever compressed, so there is no jump at any length.
  Memory holds 66 columns. The meter is sampled at 30 Hz even under Reduce
  Motion; only the redraw drops to 1 Hz.
- Done and media results are one 300×48 row with Copy on the right, as before
  the timeline.
- Every state is one 300×48 row (Linux table above uses 300×88 for results;
  the Mac no longer does). Hover buttons replace the strip inside the row.
  Done morphs in place from the transcribing pill (0.24 s, ease-out), stays
  1.75 s unless hovered, then fades out. Not pasted (`Paste`, `Copy`) and
  rescue errors stay until acted on.
- `SketchEvent` mirrors the Rust struct (`index`, `startMs`, `endMs`, `live`)
  and upserts by index. Mac draw mode sends `SketchUpdate`s through
  `recordingOverlay.applySketch(_:session:)`, which accepts changes for the
  pill's own dictation until the next one starts.
- Decode progress: FluidAudio 0.15.6 exposes `AsrManager.transcriptionProgressStream`
  only for audio longer than one model window (240,000 samples, 15 s). Koett
  subscribes only then and shows the fraction as a percentage on the right. The
  fraction counts dispatched chunks of the silence-trimmed audio, so it can run
  up to one worker batch ahead of finished decoding. Shorter dictations and
  Nemotron show the spinner only.
- Pause is available for Parakeet only. Nemotron streams live audio to its
  adapter, so the button is hidden there rather than transcribing paused audio.
- `PillPanel` returns false for `canBecomeKey` and `canBecomeMain`.
- `swift test` renders every state in both appearances when
  `KOETT_PILL_SNAPSHOT_DIR` is set (`PillSnapshotTests`). The 2026-11-02
  renders are in `docs/evidence/mac-pill-timeline-20261102/`; the fixed-width
  renders (0:05, 1:00, 5:00, 20:00, Done) are in
  `docs/evidence/mac-pill-fixed-width-20261102/`.
