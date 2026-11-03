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
