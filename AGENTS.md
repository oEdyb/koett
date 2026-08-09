# Koett Agent Guide

## Goal

Keep Koett fast, local, barebones, and easy to understand. Make the smallest
change that proves the next idea.

Koett is currently an English-only macOS dictation app for Apple Silicon. Swift
owns the macOS app. Do not rewrite it or add a cross-platform shell without a
measured Windows or Linux spike.

## Current behavior

- Toggle recording is the default. Either Option key is the default shortcut.
- Parakeet v2 is the default model and stays loaded and prewarmed.
- Nemotron 560 ms is experimental. It keeps a complete Parakeet recovery WAV.
- A small bottom-center pill shows real microphone levels and elapsed time while
  recording. It does not take keyboard focus.
- One final transcript is copied and pasted into the focused app.
- Every non-empty final transcript is appended once to
  `~/Library/Application Support/Koett/Transcripts.md`.
- Transcript entries contain the time, model, and text.
- Temporary microphone audio is deleted after transcription.
- Koett has no account, telemetry, or cloud transcription.

## Product rules

- Keep the normal recording path limited to the waveform menu, start/stop
  sounds, and the small live recording pill.
- Do not load a model for each recording.
- Post the paste event before transcript storage on the successful path. File
  storage must not delay visible text.
- Save a final transcript even if clipboard or Command-V delivery fails.
- Save each dictation once. A delivery failure must not start ASR recovery.
- Use Parakeet recovery only for Nemotron capture, streaming, or finalization
  failures.
- Do not retain audio unless Olle explicitly changes that decision.
- Do not add a database, account, cloud service, large settings screen, or live
  partial-text UI without a proven need.
- Ground API changes in official Apple documentation and pinned upstream source.
- Pin dependency versions. Benchmark and test before an upgrade changes the pin.

## Key files

- `Sources/Koett/Koett.swift`: app state, shortcut, recording, delivery, menu,
  model selection, and recovery.
- `Sources/Koett/NemotronStreamingAdapter.swift`: experimental live adapter and
  bounded ordered audio store.
- `Sources/Koett/RecordingOverlay.swift`: nonactivating live meter and timer.
- `Sources/Koett/TranscriptStore.swift`: append-only local Markdown history.
- `Tests/KoettTests/`: storage and live-audio unit tests.
- `Package.swift`: targets and exact FluidAudio version.
- `install-macos.sh`: release build, signing, Login Item update, and launch.
- `README.md`: public install, use, privacy, benchmark, and API notes.

## Required checks

Run these after code changes:

```sh
swift test
swift build -c release
git diff --check
zsh -n install-macos.sh
plutil -lint Packaging/Info.plist
```

If you install a new build, also verify:

```sh
codesign --verify --deep --strict --verbose=2 /Applications/Koett.app
dwarfdump --uuid .build/release/koett
dwarfdump --uuid /Applications/Koett.app/Contents/MacOS/koett
pgrep -alf '/Applications/Koett.app/Contents/MacOS/koett'
```

The release and installed UUIDs must match. Keep the stable Apple Development
signature so macOS does not request Accessibility approval after each build.

## Project memory

On Olle's Mac, read the canonical project card before substantial work:

`/Users/olle/Documents/Notes/Agent/04 - Business/Project Cards/Local Voice Input.md`

Record durable decisions and milestones in that card and in:

`/Users/olle/Documents/Notes/Agent/Log.md`

Do not turn this file into a session log. Keep it short and stable.
