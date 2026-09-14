## Completed feature inventory

### Core dictation

- Koett is a hidden menu-bar Login Item with no Dock icon or main window.
- A hidden Edit main menu gives the API key, model, and endpoint prompts
  working Command-X, -C, -V, and -A. Do not remove it when trimming menus.
- Toggle recording is the default. Either Option key is the default shortcut.
- Hold mode remains available from the menu.
- The menu records and saves custom dictation, media, and Ask shortcuts.
- In Toggle mode, a plain modifier tap controls dictation and a
  modifier-plus-key chord can control another feature. Hold mode rejects that
  prefix conflict because recording starts on modifier-down.
- A narrow active Core Graphics event tap receives modifier changes. AppKit
  receives ordinary key-down and key-up events. If macOS disables the event
  tap, Koett resyncs the physical modifier state before it continues.
- Modifier-only shortcuts use the event's device-specific left/right flag as
  their primary state. Do not replace this with a second
  `CGEventSource.keyState` check: that check returned `false` during real Right
  Option down events on this Mac.
- Tink and Basso sounds mark recording start and stop.
- After the stop sound, the same pill shows `Transcribing…` until the text is
  pasted or an error replaces it. A too-short recording shows nothing.
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
  **Copied** for one second. The result pill closes by itself after ten
  seconds; the menu keeps the latest result.
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
  provider controls its output limit. Length is controlled by the system prompt
  instead: answer in the first line, at most three bullets or one code block,
  80 words unless the user asks for detail, a list, or code. Temperature is 0.4.
  The prompt tells the model that the question is a raw speech-to-text
  transcript, so it reads for intent and resolves misheard words against the
  screen instead of commenting on them.
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
  the final signature, keeps the existing Login Item registered, and launches
  Koett. Any failure
  after replacement restores the previous app and Login Item.
- The installer prefers Developer ID Application, then Apple Development, then
  ad-hoc signing. It enables hardened runtime with the audio-input and Apple
  Events entitlements. The public `v0.1.1` ZIP is signed by
  `Developer ID Application: Dyberg & Co AB (LF8KF3G42Q)`, notarized, and
  stapled.
- Keep one stable signing identity. An ad-hoc or changing identity can make
  macOS request Accessibility approval again after an update.

### Recording feedback accessibility

- The recording pill is one stable `AXStaticText` element with the label
  `Koett status` and value `Recording`. Its 30 Hz waveform redraw never changes
  the accessibility value.
- Status and copied states are static text. Download progress is an
  `AXProgressIndicator` with a numeric 0...1 value and a spoken percentage.
  The media result is an `AXButton` with a labeled Copy action.
- When macOS Reduce Motion is on, Koett stops waveform level motion and updates
  only the elapsed timer at 1 Hz. Koett observes the workspace display-options
  notification so a live recording follows a settings change.
- The menu ends with a disabled `Koett <version>` item read from the bundle.
- The menu keeps the latest result or failure after its transient pill closes.
  This state is session-only; durable transcript recovery remains in the
  transcript and failed-transcript stores.
- This implementation follows Apple's current
  [AppKit custom-control guidance](https://developer.apple.com/documentation/appkit/custom-controls),
  [`NSAccessibilityProtocol`](https://developer.apple.com/documentation/appkit/nsaccessibilityprotocol),
  and
  [`accessibilityDisplayShouldReduceMotion`](https://developer.apple.com/documentation/appkit/nsworkspace/accessibilitydisplayshouldreducemotion).
  Apple does not publish an exact reference implementation for a nonactivating
  dictation pill. Koett therefore uses the smallest direct AppKit solution from
  those APIs. FluidAudio 0.15.6 is not part of this UI path and remains pinned.

### Last transcript recovery

- Koett keeps the newest nonempty completed transcript in
  `Last Transcript.txt`. It uses an atomic Foundation write, so an interrupted
  replacement does not expose a partly written file.
- Copy Last and Paste Last read this file-backed state. They never run ASR and
  never append another Markdown history row. An empty cleaned result keeps the
  prior last transcript.
- Paste Last writes the clipboard first. If Command-V cannot be created, the
  text stays on the clipboard. Menu actions wait for AppKit menu tracking to
  end, then run on the next main-loop turn so the status menu does not own the
  event target.
- This path follows Apple's current
  [`Data.WritingOptions.atomic`](https://developer.apple.com/documentation/foundation/nsdata/writingoptions/atomic),
  [`NSPasteboard`](https://developer.apple.com/documentation/appkit/nspasteboard),
  [`NSMenuDelegate.menuDidClose`](https://developer.apple.com/documentation/appkit/nsmenudelegate/menudidclose(_:)),
  and [`CGEvent.post`](https://developer.apple.com/documentation/coregraphics/cgevent/post(tap:))
  contracts. Apple does not publish an exact status-item Copy Last or Paste Last
  reference implementation. Koett uses the smallest direct combination of
  those APIs. FluidAudio 0.15.6 is not part of this path and remains pinned.
