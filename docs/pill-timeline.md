# Recording pill: timeline

Concept C from `docs/pill-concepts.html`, built on Linux on 2026-11-02. Evidence
lives in `CrossPlatform/evidence/pill-timeline-20261102/`.

## States (Linux)

Revised 2026-11-02 after Olle tried the Mac pill. Every state is exactly
300×48; nothing ever grows wider or taller.

| State | Shows | Actions | Leaves |
| --- | --- | --- | --- |
| Hidden | Nothing | none | — |
| Recording | Coral dot, rolling strip, `m:ss` | Hover swaps the strip for Cancel, Pause/Resume and History inside the same row | Stop or Cancel |
| Drawing | Coral 2 px ring, pen glyph, live band | As recording | Shift released |
| Paused | Two-bar pause glyph; the strip records silence | Hover: Resume | Resume |
| Transcribing | Spinner, frozen dimmed strip, `NN%` once the engine's segment plan is known | Hover: Cancel, History | Result |
| Done | Transcript mark, one-line result, Copy on the right | Copy | Morphs in from the decoding pill (240 ms), fades out after 1.75 s (250 ms fade); held while hovered, then 0.8 s after the pointer leaves; Copy closes it after 1 s |
| Not pasted | Warning glyph, `Not pasted · saved`, Paste and Copy on the right | Paste, Copy | Never times out; Paste closes it and pastes, Copy closes it after 1 s, or the next dictation replaces it |

After a successful paste, the decoding pill morphs in place into Done (240 ms:
the old content slides 8 px left and fades out while the new content slides in),
at the same size and position. Wayland pastes asynchronously, so Ready arrives
before the paste outcome: the decoding pill holds for up to 1 s until the outcome
arrives, so the pill never unmaps and remaps between the two.

The strip is a rolling window, the same as the Mac `PillTimeline`: the last 9.6 s
as 64 columns of 150 ms (2 px bars, 1 px gaps). It is sampled at 30 Hz while
recording, so it slides left at a constant rate even in silence. Columns are
never re-binned, so 0:05 and 20:00 look alike; memory stays at 66 columns. Time
before the start draws as dim dots, and the left six columns fade out. Sketch
bands slide with the audio and leave on the left. Reduced motion keeps sampling
and redraws once a second. The rules for both platforms live in
`docs/ui-design.md` ("Pill and overlay UI rules").

Lab evidence runs the debug build against Koett's packaged GTK runtime
(4.24.1 + upstream `21d3eb76`). The host's unpatched 4.24.1 can abort in AT-SPI
`GetChildAtIndex` when an accessibility client walks windows while one hides.

The lab shows the 20:00 strip with the debug-only `KOETT_DEBUG_PILL_TIME_SCALE=4`, which runs
only the pill's clock four times faster, using real audio.

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
