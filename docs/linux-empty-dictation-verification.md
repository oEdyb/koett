# Quiet Linux empty dictation

An empty or whitespace-only ASR result no longer overwrites the clipboard or
posts a paste, creates a blank history entry, changes Last Transcript, or shows
Not pasted. The pill returns to idle without waiting for a delivery outcome that
will never arrive. Empty input also bypasses dictionary/optional S1-mini cleanup,
so cleanup cannot invent text for silence. Nonempty transcription is unchanged.

Silent drawing/pin recordings remain supported, matching Mac completeDictation:
when the overlay has captured or pending frames, completion reaches the existing
contact-sheet composer. A marked image sheet is still delivered. If pending
captures yield no sheet or text, completion is quiet. A no-drawing empty result
invalidates its frame scope, rejecting stray late results from that session.

This does not change acoustic silence detection, infer that every quiet sound
is silence, or alter the separate empty-question/media error messages. Actual
capture/ASR failures still report errors and retain recovery as before.

## Evidence, 2026-11-04

- Baseline clean main `e4feb54`, real 110M engine, private KWin desktop, copied
  model and CPAL recording from an isolated silent Pulse source: a one-second
  silent recording replaced a seeded Unicode clipboard with empty bytes and
  showed Not pasted. Its zero-byte delivery attempt is in the private app log.
- Fixed release app, same source/model: captures stopped 0.05, 1 and 3 seconds
  after microphone opening all preserved the exact Unicode clipboard and Last
  bytes, inserted zero history rows, retained no failed WAVs and returned idle.
  These durations exclude control/poll overhead; they are not latency claims.
- Inspected native screenshots show baseline Not pasted and the fixed idle
  desktop without a pill. No host input/clipboard or installation was performed.
- 385 Rust tests passed, 17 ignored, plus native ABI subprocess check. Regression
  tests cover empty/Unicode-whitespace cleanup bypass, empty controller completion,
  rejection of late frames and the next nonempty dictation. A captured-frame test
  routes empty ASR through the real PNG contact-sheet composer and keeps its
  nonempty marked text/image result.
- Formatting, clippy with warnings denied, release build and diff check passed.
  MSVC cross-check cannot complete here without Microsoft's `lib.exe`.

Evidence: `CrossPlatform/evidence/empty-dictation-20261104/`.

Physical microphone noise, v2/v3/Nemotron live recordings, formatter service
integration and the full gesture-to-target silent-image workflow remain open.
The synthetic controller/composer test is not a real target-consumption check.
Row 6 still has implementation gaps in non-niri target identity, monitor hotplug
and early Shift clicks; deferred niri geometry/GNOME/X11 overlays remain named.

## Reproduce

Check available memory. Build in `CrossPlatform/` with CARGO_BUILD_JOBS=4,
nice -n 10 and a user scope with MemoryMax=12G. Copy a verified 110M model into
`CrossPlatform/target/silence-sol/model`, never symlink the live model. From root:

```sh
agent-lab start koett-silence-sol
systemd-run --user --scope -p MemoryMax=12G nice -n 10 \
  agent-lab run koett-silence-sol -- python3 \
  "$PWD/CrossPlatform/evidence/empty-dictation-20261104/check.py" "$PWD" \
  /path/to/clean-e4feb54-release/koett
agent-lab stop koett-silence-sol
```

The fixture refuses other lab runtimes, recreates only its own baseline/fixed
scenario directories below target/silence-sol, and restores its private audio
module after guarded Quit. The baseline binary is read-only and never installed.
