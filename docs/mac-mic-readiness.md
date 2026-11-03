# Mac microphone hardening evidence

The microphone branch is based on main after PR #36. Baseline for timing is the
previously installed source, `ee6f87b`. All probes use disposable files and
preferences, without microphone capture or new permission requests. Model caches
are shared read-only during inference. No personal history is used.

## What the checks establish

- Full Swift tests, release build, packaging syntax and plist checks are required.
- Shortcut tests run the controller event path and guard the actual event-tap
  mask. Right Option recovery runs the controller in Hold mode with a false
  physical key query and a true right-device flag.
- Channel tests cover the WAV sink and Nemotron live audio store. Nemotron tests
  put signal on each input of 2/4/8/32-channel formats, preserve silence, and
  average two active channels. They do not test physical interfaces or the
  Nemotron model.
- No audio uses the existing error renderer, fixed at 300×48. Off-screen snapshots
  cover both appearances and the other pill states. No new visual treatment.
- Quit tests use an injected recording and temporary stores. Force-kill probes
  kill only an isolated XCTest child while its real WAV sink is open; they do
  not crash the installed app.

## Timing scope

`KOETT_READINESS_LAB=/tmp/koett-mic-lab swift test -c release --filter MacReadinessProbeTests`
loads cached Parakeet v2, warms it with 0.3 seconds of silence, transcribes synthetic
Samantha speech and 10/30-minute repetitions, writes a private named clipboard,
and persists under the test folder. The probe writes `readiness.json`,
`crash.json` and the transcripts there.

Model startup is measured inside the test process, excluding app launch,
permissions, menu/hotkey setup and optional dictionary loading. The delivery
measurement ends at an injected successful paste callback using that private
clipboard. It is **not real stop-to-paste latency**: microphone finalization,
Command-V posting and target app consumption are not measured. Do not label it
as such in a video. The long fixtures are offline audio, not real-time capture
endurance. A before/after comparison with these limits cannot establish that the
whole app got faster.

## Dictionary import

The installed app quit cleanly before `koett --import-dictionary
/absolute/path/to/docs/dictionary-seed.txt` ran. Count changed from 17 to 26:
all 20 starter terms plus six existing terms outside the seed. The import added
nine seed terms. A comparison confirmed every prior term and form remained;
Scrawl has no `scroll` form. The app was restarted afterward.

## Measured before/after

Three cached-model process runs per revision, alternating order, on the same Mac
and identical synthetic fixtures. These are medians, not a claim of speedup;
background machine load was not controlled. Raw runs and fixture hashes are in
[`docs/evidence/mac-mic-readiness`](evidence/mac-mic-readiness).

- Before (ee6f87b): model load + prewarm 265.8 ms; 10.459 s clip ASR 106.9 ms; 10 min ASR 2.741 s; 30 min ASR 6.708 s.
- After (9830dfa): model load + prewarm 269.6 ms; 10.459 s clip ASR 120.3 ms; 10 min ASR 2.631 s; 30 min ASR 6.788 s.

Final before/after transcripts match byte for byte: 33, 1,892 and 5,680 words.
This is output parity, not a scored accuracy measurement against a reference.
Both revisions' force-killed sink files were 4,096 bytes with zero readable
frames. This confirms that force quit is unsafe during capture; it does not
establish how much audio a later crash would retain.

Accelerated capture tests separately write exactly 9,600,000 and 28,800,000 frames
through the production WAV sink (10 and 30 minutes at 16 kHz), with pacing and
final frame/error assertions. They exercise sustained file volume, not real-time
hardware or 40 minutes of user speech.

All 40 light/dark pill snapshots were inspected, including No audio and morph
frames. The no-audio state fits the existing row. A live try of the installed
microphone, shortcut and target app remains required before filming.

## Release gates

Final full `swift test`: **382 tests, 5 intentional skips, 0 failures**. It ran with `KOETT_READINESS_LAB` and
`KOETT_PILL_SNAPSHOT_DIR` set, enabling crash, model/long-audio, accelerated
capture and off-screen visual probes. The isolated long-capture test passed
in 50.393 seconds; both files contained the exact expected frame counts.
The deliberately disabled-fix run failed all three named regression tests;
restoring the fixes passed the 23-test focused suite. Sources were restored
before the final full run. No independent review was run.

Final `swift build -c release` passed in 68.87 seconds. `git diff --check`,
`zsh -n install-macos.sh`, and both packaging plist checks passed.
