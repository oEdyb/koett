# Koett Agent Guide

## Goal

Keep Koett fast, local, barebones, and easy to understand. Make the smallest
change that proves the next idea.

For platform code and unfamiliar APIs, do not guess. First reproduce the
problem and collect the exact error. Then read the current official platform
documentation, the pinned library documentation, and a real reference example
when one exists. Trace the root cause before editing code. Implement the
smallest documented fix and verify the original user flow on the real platform.
Do not ship a speculative workaround.

Koett stays a voice-to-text app first. Ask is optional and must not complicate,
slow, or require cloud access for normal dictation.

Koett is currently an English-only macOS dictation app for Apple Silicon. Swift
owns the macOS app. Windows and Linux use the separate native Rust app under
`CrossPlatform/`; do not rewrite the working macOS app.

## Implementation status — 2026-08-24

The native Rust Windows and Linux v0.1 implementation lives under
`CrossPlatform/` on `feat/windows-linux-v0.1`. It is one small process with
CPAL 0.18.2, ringbuf 0.5.1, and statically linked sherpa-onnx 1.13.5. It provides
toggle recording, one warmed local Parakeet 110M engine, paste, Markdown
history, a configurable shortcut, start at login, a tray menu, single-instance
protection, first-run model progress, and visible failures. Windows uses Win32.
Linux uses X11 APIs on X11 and XDG portals on Wayland. The macOS app is
unchanged. Fedora 44 GNOME Wayland and Windows 11 now pass the real
microphone-to-paste-and-history flow. A real X11 desktop test, Windows
keyboard-only tray test, and one long-recording test in the real Windows app
are still required before either port ships.

Native CI also runs `koett-engine --self-test` on fresh Windows x86-64, Linux
x86-64, and Linux ARM64 VMs. The self-test uses the desktop app's real pinned
downloader and SHA-256 checks, loads Parakeet plus Silero VAD, and transcribes
the official 7.435-second model sample. Windows also runs a 420-second,
44.1 kHz long-input regression with a short utterance inside one minute of
silence. [CI run 32765934319](https://github.com/oEdyb/koett/actions/runs/32765934319)
passed at implementation commit `254a1b8`: 31 Windows tests, 29 common host
tests, strict Clippy, release builds, fresh-model inference, packaging, and the
long regression. Windows transcribed the 420-second fixture in 12.416 seconds
and kept every required repeated-speech anchor plus the isolated short
utterance. This proves model setup and bounded local inference. It does not
prove microphone, tray, shortcut, portal, or focused-app paste behavior.

Recordings up to 20 seconds keep the original one-shot decoder. Longer audio
is resampled to 16 kHz while recording. One dedicated worker owns the warmed
model and decodes completed official Silero VAD segments in the background.
No model is copied or reloaded. The full microphone recording stays available
until final success so any background error can use the existing bounded
fallback. Custom Parakeet folders without the VAD file still use that fallback.
The default installer downloads the 643,854-byte official `silero_vad.onnx`
with exact size and SHA-256 verification. On the M5 host, the final production
420-second, 44.1 kHz self-test kept every speech anchor, left 84.7 ms after stop,
and used 450,363,392 bytes maximum RSS. The final direct and worker paths for
the 7.435-second sample measured 228.7 ms and 197.8 ms, so the normal short path
has no measured latency cost.

All direct Rust dependencies were checked against crates.io and their upstream
documentation on 2026-08-25. Keep sherpa-onnx at 1.13.5 because 1.13.6 has no
relevant ASR fix and did not prove faster. Keep x11rb at 0.13.2 until
global-hotkey can move with it; using 0.14.0 now adds a second x11rb copy for no
Koett-relevant fix.

Cross-platform history writes use private files. If the main history fails,
Koett writes one collision-safe file under the durable `Failed Transcripts`
data folder. A stalled first-run download checks cancellation at least every
five seconds. Optional start-at-login failures warn the user but never block
dictation. Linux requires a working StatusNotifier/AppIndicator tray host so it
cannot run with all controls and errors hidden.

- Public `main` contains the current Mac source described below: core dictation,
  configurable shortcuts, transcript recovery, optional media transcription,
  optional local formatting, and optional Ask with spoken replies.
- Public release `v0.1.0` provides the notarized Apple-silicon app at
  `https://github.com/oEdyb/koett/releases/tag/v0.1.0`. It targets exact source
  commit `10337fc9eeede3679ebc122a56f09197b86b01bd`. Apple accepted submission
  `a37adefa-a412-473c-9444-d60257efbc6a`; the ticket is stapled, and host
  Gatekeeper reports `source=Notarized Developer ID`. The release ZIP SHA-256
  is `1f6a8eff4f0facc6e1018338781dfbfef15ac20987a21f14fd6697259630d6ae`.
- The current signed release and installed app match UUID
  `30AB1997-EE20-3334-88C1-29B72CBFB7CD`. The Login Item is running. Olle
  live-confirmed no repeated Keychain prompt, clean text on the glass, and
  Katie speech on 2026-08-11. On 2026-08-23, Olle live-confirmed the final
  release-readiness install by dictating and pasting `Hello, hello, hello.`
- A clean Tart macOS 26.6.2 VM passed the source-install path on 2026-08-23.
  The first run fetched FluidAudio 0.15.5, requested Microphone and
  Accessibility correctly, started at login after a reboot, downloaded the
  451 MB Parakeet cache with visible progress, recorded real audio, pasted the
  result into TextEdit, copied it to the clipboard, and saved it to
  `Transcripts.md`. A second pristine clone fetched the final FluidAudio 0.15.6
  pin, built in 149.75 seconds, installed, launched one process, passed strict
  signature verification, and matched its guest release UUID
  `E87B9F09-B5E0-37A2-BE7B-0A11D13B8D2F`. The small VM image has Command Line
  Tools but no Xcode `XCTest` module, so run the 53-test suite on the host.
- `Koett.swift` was reduced from about 1,800 lines to 741. Startup, menus,
  dictation, media, and Ask now have focused files. Keep this simple split;
  do not add a framework, service container, or generic plugin system.

## Completed feature inventory

### Core dictation

- Koett is a hidden menu-bar Login Item with no Dock icon or main window.
- Toggle recording is the default. Either Option key is the default shortcut.
- Hold mode remains available from the menu.
- The menu records and saves custom dictation, media, and Ask shortcuts.
- In Toggle mode, a plain modifier tap controls dictation and a
  modifier-plus-key chord can control another feature. Hold mode rejects that
  prefix conflict because recording starts on modifier-down.
- A narrow active Core Graphics event tap receives modifier changes. AppKit
  receives ordinary key-down and key-up events. If macOS disables the event
  tap, Koett resyncs the physical modifier state before it continues.
- Tink and Basso sounds mark recording start and stop.
- A nonactivating bottom-center pill shows real microphone levels and elapsed
  recording time. It stays above apps without taking keyboard focus.
- Koett records a temporary 16 kHz mono Float32 WAV. Audio is deleted after
  transcription or recovery.
- If local ASR fails, Koett moves the WAV to
  `~/Library/Application Support/Koett/Failed Recordings/`, shows a visible
  failure, and adds **Open Failed Recordings** to the menu. Successful ASR still
  deletes audio, including clipboard or paste failures.
- If the main transcript history cannot be written, Koett saves the completed
  text as a separate file under
  `~/Library/Application Support/Koett/Failed Transcripts/`.
- One final transcript is copied and pasted into the focused app with
  Command-V. Koett does not show live partial dictation text.
- Every non-empty final transcript is appended exactly once to
  `~/Library/Application Support/Koett/Transcripts.md` with its time, model,
  and text. Storage must not delay the visible paste.
- Koett has no account or telemetry.

### Local speech engines

- Parakeet v2 through FluidAudio `0.15.6` is the default. It downloads once,
  stays loaded, and is prewarmed at startup. Never reload it per recording.
- Nemotron Streaming EN 0.6B at the 560 ms configuration is an experimental
  menu option. It processes microphone audio while the user speaks.
- Every Nemotron recording also keeps a complete temporary Parakeet recovery
  WAV. Use that recovery only after capture, streaming, or finalization fails.
- Model changes restart Koett. Command-line `--parakeet` and `--nemotron`
  flags override the saved model for one launch.
- The repo includes standalone audio, corpus, noise, Apple Speech, Parakeet,
  and Nemotron benchmark executables. These harnesses stay separate from the
  small production app path.
- Raw Parakeet text remains the default. S1-mini by Superwhisper is an optional
  local cleanup mode because every tested quantization and style removed
  uncertainty in at least one meaning test.
- `Formatting: Raw…` or `Formatting: S1-mini…` opens one small transient AppKit
  popover. It stays open while the user changes Output, Styling, Structure, and
  Context, then closes on Escape or an outside click. The Output control alone
  enables or disables S1-mini; formatting choices can be prepared while Raw is
  active.
- The popover exposes all official S1-mini controls: Styling (Casual,
  Semi-casual, Semi-formal, Formal), Structure (Prose, Lists), and Context
  (General, Email).
- S1-mini uses the official Q4_K_M GGUF, exact control line, thinking disabled,
  greedy decoding, and the measured N7/M48 llama.cpp configuration. The first
  enable downloads and verifies the 462 MiB model. `llama-server` must be
  installed through `brew install llama.cpp`.
- Koett keeps S1-mini loaded while the mode is active. Any cleanup error falls
  back to raw text. History stores both the raw and cleaned transcript. The
  benchmark and full measurements are in `Benchmarks/S1Mini/`.

### Focused media transcription

- The default shortcut is Control-Shift-T and can be rebound.
- Koett reads the focused Chrome or Safari page URL and gives one HTTP or HTTPS
  item to `yt-dlp`. YouTube, TikTok, Instagram, X, Vimeo, Facebook, Twitch, and
  SoundCloud get clear source labels; other supported sites use their hostname.
- `yt-dlp` and FFmpeg fetch and convert temporary audio. `--no-playlist` plus
  `--playlist-end 1` bounds the request to one item. Koett does not read browser
  cookies. The already-warm local Parakeet model transcribes the audio in the
  background, then Koett deletes it.
- Koett rejects live media and media longer than three hours, caps the fetched
  source at 2 GiB, uses a 30-second network socket timeout, and stops the full
  fetch after 30 minutes. Task cancellation also terminates the child process.
- Each result is saved as a dated Markdown file under
  `~/Library/Application Support/Koett/Media Transcripts/` with its source URL,
  title, and transcript.
- The existing pill shows fetch and transcription progress. The compact 250×48
  result uses Koett's coral waveform, one normalized transcript preview line
  with a native trailing ellipsis, and a labeled `doc.on.doc` **Copy** button.
  The button has hover and pointer states, then shows a coral checkmark plus
  **Copied** for one second.
- Do not bundle third-party platform logos in the first version. Their trademark
  rules differ; TikTok requires prior written permission, and YouTube's in-app
  icon rules require a link back to YouTube content. Known platform names use
  exact root-domain or subdomain boundaries for saved metadata. Unknown sites
  use their hostname. Spoofed lookalike hosts never receive a known label.
- A live user-owned 4:11 video fetched and converted in 2.19 seconds. Warmed
  Parakeet ASR took 0.646 seconds at 388.8x real time with 0.973 confidence.
- Platform terms and source rights remain distribution constraints. Describe
  this as user-directed processing for media the user owns or may process. Use
  “supported media”; do not promise that every page or protected item works.

### Koett Ask

- The default shortcut is Control-Shift-Space and can be rebound.
- The first press starts a local voice question and captures one JPEG from the
  active display. The second press stops recording and sends the request.
- Ask uses the warmed local Parakeet model for the question. It appends that
  question to the normal transcript history.
- ScreenCaptureKit keeps one image in memory. Koett does not save it. The panel
  says **Screen included** so the cloud boundary is visible.
- Ask streams the response into a right-side floating panel that stays above
  apps, spaces, Stage Manager, and full-screen apps. It can see the captured
  frame but cannot click, type, or control other apps.
- The provider-neutral client supports Groq, OpenRouter, and a custom
  OpenAI-compatible HTTPS chat-completions endpoint.
- Current defaults are Groq `qwen/qwen3.6-27b`, OpenRouter
  `~openai/gpt-latest`, and no custom model. The menu can change the provider,
  model, custom endpoint, and API key.
- The request does not set `max_tokens` or `max_completion_tokens`. The selected
  provider controls its output limit.
- Provider API keys stay in macOS Keychain. Never put keys in source,
  `AGENTS.md`, README, logs, screenshots, commits, or test fixtures.
- The local renderer supports sanitized GFM, headings, lists, task lists,
  quotes, links, tables, emphasis, strike-through, inline code, highlighted
  fenced code, and inline or block LaTeX math.
- Marked `18.0.10`, KaTeX `0.18.4`, highlight.js `11.12.0`, and DOMPurify
  `3.4.14` are pinned inside the app. A strict Content Security Policy blocks
  runtime network access. WebKit uses a nonpersistent data store.
- The WebKit answer surface disables its opaque background and uses a clear
  under-page color so Markdown appears directly on Liquid Glass. A snapshot
  regression test checks that an unused answer pixel has alpha below `0.05`.
- Pressing Ask while a response is running cancels it and starts a new question.
- Normal dictation does not use Ask, ScreenCaptureKit, WebKit, or a cloud model.

### Optional spoken Ask replies

- Cartesia Sonic 3.5 is the first TTS adapter. Speech is optional and off by
  default for public installs.
- The Cartesia key stays in macOS Keychain. The menu selects Katie, Skylar,
  Jameson, Gemma, or Archie.
- Ask opens one Cartesia WebSocket while the user is still recording. This
  hides the connection setup behind local question capture and transcription.
- Complete response sentences stream to Cartesia while the full text answer
  continues to render. AVFoundation plays raw 44.1 kHz Float32 PCM.
- Koett speaks at most the opening paragraph or three sentences. It strips
  Markdown and URLs and stops before code fences or detailed sections.
- Starting dictation, media transcription, or another Ask stops speech
  immediately. A TTS failure must never stop or hide the text answer.
- The stable model ID is `sonic-3.5-2026-05-04`. A real persistent-WebSocket
  benchmark produced first audio in 96.5, 83.3, 84.6, 86.1, and 81.8 ms after
  each first sentence was sent. Socket setup took 126.5 ms and was hidden
  behind recording. This passes the sub-300 ms first-audio product gate.

### Packaging and permissions

- `/Applications/Koett.app` uses bundle ID `com.olledyberg.Koett`, runs as an
  accessory app, and registers through `SMAppService.mainApp`.
- Koett requires Microphone and Accessibility for normal dictation. Media tab
  detection also needs browser automation access when macOS requests it. Ask
  needs Screen Recording.
- First run uses the existing pill and menu to show permission checks, exact
  model download percentages, model load, warm-up, ready state, and short
  errors. Permission errors include direct System Settings actions and Retry.
- `install-macos.sh` builds and verifies a staged release app before it stops
  the installed app. It then makes an atomic same-volume replacement, verifies
  the final signature, updates the Login Item, and launches Koett. Any failure
  after replacement restores the previous app and Login Item.
- The installer prefers Developer ID Application, then Apple Development, then
  ad-hoc signing. It enables hardened runtime with the audio-input and Apple
  Events entitlements. The public `v0.1.0` ZIP is signed by
  `Developer ID Application: Dyberg & Co AB (LF8KF3G42Q)`, notarized, and
  stapled.
- Keep one stable signing identity. An ad-hoc or changing identity can make
  macOS request Accessibility approval again after an update.

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
- `CrossPlatform/src/transcription/background.rs`: warmed Rust model worker and
  background long-recording path.

## Verification receipts

- The current source passes 53 tests with zero failures. Setup tests cover
  model-progress normalization and permission messages. Failure tests cover
  visible paste/microphone errors and collision-safe failed-audio recovery.
  Media tests cover
  the original URL, one-item and no-cookie arguments, process timeout and output
  cleanup, common user-facing errors, and collision-safe transcript storage.
- A live Accessibility-driven UI test opened the installed Formatting popover,
  changed Styling, Structure, and Context in sequence, and confirmed that the
  same popover remained present after every choice. Escape closed it. Raw stayed
  active and `llama-server` stayed stopped.
- The WebKit integration test renders a table, KaTeX equation, and highlighted
  Swift while removing scripts and event handlers.
- The fast-stream renderer test proves visible intermediate output while token
  deltas arrive every 5 ms.
- `swift build -c release`, `git diff --check`, installer shell syntax, and both
  packaging plist validations pass for the current source.
- The installed app passes strict code-signature verification with hardened
  runtime and the expected entitlements. Its executable UUID matches the
  release executable at `30AB1997-EE20-3334-88C1-29B72CBFB7CD`.
- Olle live-confirmed the installed core path with `Hello, hello, hello.` after
  the final signed replacement.
- The focused-media expansion passed a read-only Swift review after fixes for
  full process-group termination and concurrent transcript-name collisions. No
  Critical or Important issue remains in the scoped media files.
- Direct no-cookie extraction and local ASR passed public TikTok, Instagram, X,
  Facebook, Twitch, and SoundCloud fixtures. The installed app passed the full
  focused Safari TikTok path. The compact transcript-preview result, labeled
  Copy action, copied state, and clipboard output were inspected live.
- Olle live-confirmed the repaired Ask path: no repeated Keychain prompt, clean
  transparent text, and audible Katie speech. Katie then stopped speaking; the
  current design intentionally stops at the opening paragraph or three
  sentences. Treat a mid-sentence cutoff as a bug if it reproduces.
- A same-Mac five-run comparison measured Koett at 118.5 ms median from release
  to paste-post and Wispr Flow at 481 ms median through its finished-processing
  state. The supported claim is only: “about four times faster than Wispr Flow
  in my test on my M5 Mac.” Do not claim universal superiority.
- The cross-platform core passes 23 unit tests and strict Clippy checks on the
  host and for `x86_64-pc-windows-msvc`. A Debian container passes the Linux
  tests, strict Clippy checks, and full release link. Actionlint passes the
  Windows 2025 and Ubuntu 22.04 artifact workflow. It packages Windows x86-64
  plus Linux x86-64 and ARM64. Fresh native CI VMs also download, verify, load,
  and run the pinned model against its official sample. Native CI run
  `32737928880` is green at `25e2aed` on all three targets.
- Fedora 44 ARM64 GNOME Wayland passes the real global-shortcut, microphone,
  local Parakeet, automatic-paste, and saved-history flow.
- The exact Windows x86-64 CI artifact at `25e2aed` passes the real Windows 11
  recording UI, microphone, local Parakeet, automatic-paste, saved-history,
  one-process, and start-at-login registry checks. The successful five-second
  capture had RMS `0.047456` and peak `0.499985`; recoverable WASAPI `Xrun`
  notices did not abort it. The executable SHA-256 is
  `4ffd96436b79e51a4604e4ff8da2534b111a05985f2dbc0e21f93235b971680d`.

## Known unfinished work

- Vimeo currently fails before download because yt-dlp's anonymous macOS OAuth
  client returns HTTP 401. Koett intentionally does not import browser cookies.
- Live-confirm rich Markdown and math in the installed panel.
- The public binary release is Apple silicon only. There is no Intel Mac build.
- Windows and Linux have complete v0.1 implementation branches, but they are
  not released platforms yet. The bounded background path now passes the native
  420-second regression, but it still needs one long recording through the real
  Windows microphone UI. Also verify the Windows tray through keyboard
  activation on a real keyboard and run one X11 desktop test before merging or
  promising three-platform support.
- The Linux artifact targets glibc 2.35 or newer and needs the system ALSA
  runtime. Wayland also needs the GlobalShortcuts, RemoteDesktop, and Clipboard
  desktop portals. A GNOME tray needs AppIndicator support.

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

For changes under `CrossPlatform/`, also run:

```sh
cd CrossPlatform
cargo fmt --check
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
cargo check --locked --target x86_64-pc-windows-msvc
```

## Project memory

On Olle's Mac, read the canonical project card before substantial work:

`/Users/olle/Documents/Notes/Agent/04 - Business/Project Cards/Local Voice Input.md`

Record durable decisions and milestones in that card and in:

`/Users/olle/Documents/Notes/Agent/Log.md`

Keep this as a current-state handoff, not a chronological session log. Replace
stale status when the app changes instead of appending duplicate progress notes.
