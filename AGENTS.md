# Koett Agent Guide

## Goal

Keep Koett fast, local, barebones, and easy to understand. Make the smallest
change that proves the next idea.

Koett stays a voice-to-text app first. Ask is optional and must not complicate,
slow, or require cloud access for normal dictation.

The public Koett app is an English-only macOS dictation app for Apple Silicon.
Swift owns the macOS app. A separate draft branch contains the measured native
Windows/Linux implementation and the optional multilingual Mac model. Do not
rewrite the working Mac app or add a cross-platform shell.

## Takeover snapshot — 2026-09-14

- Mac worktree: `/Users/olle/dev/local-voice-input`, branch `main`. Its latest
  product commit is `fd83546` (agent transcription CLI, feature contracts split
  into `docs/`). Below it sit the unpushed local commits for recording-feedback
  accessibility (`b94bd4d`) and durable Copy/Paste Last Transcript recovery
  (`adf55db`). None of these are pushed or released.
- Public state: `origin/main` is `5384daa`. The latest public release is
  notarized Apple-silicon `v0.1.1` from source commit `c72f834`.
- Cross-platform worktree: `/Users/olle/dev/koett-cross-platform`, branch
  `feat/windows-linux-v0.1`, head `e85d0d0`. The branch matches its remote and
  backs draft PR #2. GitHub reports that the draft PR conflicts with `main`.
- The cross-platform worktree also contains uncommitted KTT-002 work. It builds
  sherpa-onnx `v1.13.5` from pinned source with TTS disabled and patches only
  the Rust native link boundary. Preserve that worktree. Do not clean, reset,
  rebase, or regenerate its evidence before reading its own `AGENTS.md`.
- The installed `/Applications/Koett.app` was built from `a3e604e` on
  2026-09-14 with `install-macos.sh`, UUID `58DF3CA5-1D06-3BE2-B81C-D9C7D876DF24`,
  Apple Development signature, `parakeet-v2` selected, S1-mini off. Its
  internal version is `0.1.1`, but it is not the public notarized binary.
- The `koett-transcribe` CLI is installed as a symlink at
  `~/.local/bin/koett-transcribe` into this checkout; see
  `docs/transcribe-cli.md`.
- Do not merge PR #2, publish a release, change the default model, or discard
  either worktree's local state without Olle's explicit approval.

## Implementation status — 2026-09-02

- Public `main` contains the released Mac source described below: core
  dictation, configurable shortcuts, transcript history, optional media
  transcription, optional local formatting, and optional Ask with spoken
  replies. The two local commits above add accessibility and last-result
  recovery on top of that public state.
- Public release `v0.1.1` provides the notarized Apple-silicon app at
  `https://github.com/oEdyb/koett/releases/tag/v0.1.1`. It targets exact source
  commit `c72f83415b1d760c30316f07fc776f09b0ebdc0b`. Apple accepted submission
  `175a1f2c-32d8-49f7-b6c5-f1bc2e0a5c8c`; the ticket is stapled, and host
  Gatekeeper reports `source=Notarized Developer ID`. The release ZIP SHA-256
  is `980fb0cc7db7f8060298912393b6e5c4904d6022005e94a4ed0cb6c8c08225f0`.
- The public signed release has UUID
  `CEDFCAD1-74F2-3357-8AAB-4F4643C293C3`. Olle
  live-confirmed no repeated Keychain prompt, clean text on the glass, and
  Katie speech on 2026-08-11. On 2026-08-23, Olle live-confirmed the final
  release-readiness install by dictating and pasting `Hello, hello, hello.`
  The same live phrase verified the Right Option repair before v0.1.1 shipped.
- A clean Tart macOS 26.6.2 VM passed the source-install path on 2026-08-23.
  The first run fetched FluidAudio 0.15.5, requested Microphone and
  Accessibility correctly, started at login after a reboot, downloaded the
  451 MB Parakeet cache with visible progress, recorded real audio, pasted the
  result into TextEdit, copied it to the clipboard, and saved it to
  `Transcripts.md`. A second pristine clone fetched the final FluidAudio 0.15.6
  pin, built in 149.75 seconds, installed, launched one process, passed strict
  signature verification, and matched its guest release UUID
  `E87B9F09-B5E0-37A2-BE7B-0A11D13B8D2F`. The small VM image has Command Line
  Tools but no Xcode `XCTest` module, so run the host test suite with Xcode.
- `Koett.swift` was reduced from about 1,800 lines to 741. Startup, menus,
  dictation, media, and Ask now have focused files. Keep this simple split;
  do not add a framework, service container, or generic plugin system.

## Feature contracts

Before changing dictation, speech engines, transcription, Ask, packaging, or feedback, read the corresponding section of [feature contracts](docs/feature-contracts.md). Preserve its runtime and recovery invariants; unrelated feature sections need not load.

## Defaults and local data

| Item | Default or location |
|---|---|
| Dictation mode | Toggle |
| Dictation shortcut | Either Option key |
| Media shortcut | Control-Shift-T |
| Ask shortcut | Control-Shift-Space |
| Local ASR | Parakeet v2 |
| Dictation text | Raw; optional S1-mini cleanup |
| Ask provider | Groq |
| Ask model | `qwen/qwen3.6-27b` |
| Spoken replies | Off for public installs |
| Spoken voice | Katie |
| Dictation history | `~/Library/Application Support/Koett/Transcripts.md` |
| Last transcript | `~/Library/Application Support/Koett/Last Transcript.txt` |
| Media history | `~/Library/Application Support/Koett/Media Transcripts/` |
| Failed ASR audio | `~/Library/Application Support/Koett/Failed Recordings/` |
| Failed history writes | `~/Library/Application Support/Koett/Failed Transcripts/` |
| Provider and Cartesia keys | macOS Keychain |
| Microphone and media audio | Temporary; deleted after processing |
| Ask screen image | Memory only; never saved |

Only the final Ask question and one screen image go to the selected assistant
provider. Only the short spoken opening goes to Cartesia when speech is enabled.
Dictation and all audio transcription stay local.

## Product rules

- Keep the normal recording path limited to the waveform menu, start/stop
  sounds, and the small live recording pill.
- Do not load a model for each recording.
- Keep raw dictation as the safe default. S1-mini must remain optional and must
  fall back to raw text after any cleanup failure.
- Post the paste event before transcript storage on the successful path. File
  storage must not delay visible text.
- Save a final transcript even if clipboard or Command-V delivery fails.
- Save each dictation once. A delivery failure must not start ASR recovery.
- Use Parakeet recovery only for Nemotron capture, streaming, or finalization
  failures.
- Retain audio only when local ASR fails. Delete it after successful ASR, even
  if clipboard or paste delivery then fails.
- Do not add a database, account, large settings screen, continuous screen
  capture, or live partial-text UI without a proven need.
- Ground API changes in official Apple documentation and pinned upstream source.
- Pin dependency versions. Benchmark and test before an upgrade changes the pin.

## Key files

- `Sources/Koett/Koett.swift`: shared app state, startup, shortcut routing, and
  settings actions.
- `Sources/Koett/KoettApp.swift`: app entry point and Login Item setup.
- `Sources/Koett/KoettMenu.swift`: menu-bar menu construction.
- `Sources/Koett/DictationSession.swift`: recording, local transcription,
  paste delivery, and Nemotron recovery.
- `Sources/Koett/SetupStatus.swift`: small first-run status and download-progress
  presentation model.
- `Sources/Koett/FailedRecordingStore.swift`: collision-safe failed-ASR audio
  recovery folder.
- `Sources/Koett/FailedTranscriptStore.swift`: fallback storage for completed
  text when the main history cannot be written.
- `Sources/Koett/DictationFailurePresentation.swift`: short visible failure
  messages.
- `Sources/Koett/S1MiniCleaner.swift`: optional local cleanup runtime, verified
  model download, official formatting controls, and settings actions.
- `Sources/Koett/FormattingPopover.swift`: persistent multi-setting AppKit
  popover anchored to the status item.
- `Sources/Koett/MediaSession.swift`: focused browser-media transcription workflow.
- `Sources/Koett/NemotronStreamingAdapter.swift`: experimental live adapter and
  bounded ordered audio store.
- `Sources/Koett/RecordingOverlay.swift`: nonactivating live meter and timer.
- `Sources/Koett/ShortcutBinding.swift`: shortcut matching, persistence values,
  labels, and the small shortcut recorder.
- `Sources/Koett/BrowserMedia.swift`: focused browser URL and temporary media
  audio fetch through `yt-dlp` and FFmpeg.
- `Sources/Koett/MediaTranscriptStore.swift`: one Markdown file per media
  transcript.
- `Sources/Koett/AssistantClient.swift`: provider-neutral streaming chat client.
- `Sources/Koett/AssistantConfiguration.swift`: provider endpoints and models.
- `Sources/Koett/AssistantAPIKeyStore.swift`: Keychain-backed provider keys.
- `Sources/Koett/AssistantPanel.swift`: right-side SwiftUI/AppKit floating panel.
- `Sources/Koett/AssistantSession.swift`: Ask recording, answer, interruption,
  provider, and voice orchestration.
- `Sources/Koett/CartesiaSpeechOutput.swift`: sentence-buffered streaming TTS
  and native audio playback.
- `Sources/Koett/AssistantMarkdown.swift`: local WebKit Markdown and math bridge.
- `Sources/Koett/Resources/AssistantRenderer/`: pinned local renderer assets.
- `Sources/Koett/ScreenContextCapture.swift`: one in-memory screen image.
- `Sources/Koett/TranscriptStore.swift`: append-only local Markdown history.
- `Tests/KoettTests/`: shortcut, storage, media, renderer, client, TTS-buffer,
  live-audio, and S1-mini request tests.
- `Benchmarks/S1Mini/`: isolated S1-mini corpus, harness, and measured decision.
- `Package.swift`: targets and exact FluidAudio version.
- `install-macos.sh`: release build, signing, Login Item update, and launch.
- `README.md`: public install, use, and privacy.

## Prior verification

[Verification receipts](docs/verification-receipts.md) record historical evidence. Read the relevant receipt for a regression investigation or release comparison. Current changes still need the checks below; old receipts are not new test results.

## Known unfinished work

- Vimeo currently fails before download because yt-dlp's anonymous macOS OAuth
  client returns HTTP 401. Koett intentionally does not import browser cookies.
- Live-confirm rich Markdown and math in the installed panel.
- The public binary release is Apple silicon only. There is no Intel Mac build.
- Windows and Linux are implemented on draft branch
  `feat/windows-linux-v0.1`, but they are not released platforms. Finish the
  ASR-only native build and artifact gates, reconcile the conflicting draft PR,
  then complete the remaining real Windows, Linux, packaging, accessibility,
  recovery, and clean-install checks before promising three-platform support.

## Required checks

Run these after code changes:

```sh
swift test
swift build -c release
git diff --check
zsh -n install-macos.sh
plutil -lint Packaging/Info.plist
plutil -lint Packaging/Koett.entitlements
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

Keep this as a current-state handoff, not a chronological session log. Replace
stale status when the app changes instead of appending duplicate progress notes.
