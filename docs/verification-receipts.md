## Verification receipts

- A fresh 2026-09-02 run passed all 68 tests with zero failures.
  Last-transcript tests
  cover missing, empty, corrupt, exact-copy, clipboard-failure, paste-failure,
  no-duplicate-history, and one-shot post-menu-close behavior. Recording-overlay
  tests snapshot every accessibility state, prove that waveform refreshes do
  not change the accessibility representation, and prove the 1 Hz Reduce
  Motion policy. Setup tests cover
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
  release executable at `CEDFCAD1-74F2-3357-8AAB-4F4643C293C3`.
- Olle live-confirmed the installed core path with `Hello, hello, hello.` after
  the final signed replacement.
- Olle live-confirmed the repaired Right Option toggle path with
  `Hello, hello, hello.` on 2026-08-25. The stable Apple Development build is
  installed with executable UUID `CEDFCAD1-74F2-3357-8AAB-4F4643C293C3`.
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

### Linux host install — 2026-11-04

Clean main `3b4785feb501af658e7d53c7b856a6c764b1d91c` (#70/#71/#72 plus the
previous Linux batches) is installed as a private current-PC bundle. Its running
executable matched candidate SHA-256
`235a69538abe5a6fe9f1bc6ec153ce6a434ea69efd4ca9a2cda79a1ca4495933`.
Koett was idle, exited through guarded Quit, and was confirmed absent before
the installer switched the launcher. All four configuration/saved-data file
hashes were unchanged through installation and restart. Model, formatting and
paste preferences were preserved; existing runtime and previous versions were
retained. The transcription CLI link and help were checked.

Clean-main release build and 392 tests passed (17 ignored, plus native ABI check).
Required-runtime preflight passed. Optional rich Ask is reported unavailable
(the complete WebKitGTK runtime/helpers are required); its plain-text fallback
is retained. The new background process reached busy false. No new live host
microphone, pin or physical monitor acceptance is claimed. The optional niri
adapter in PR #73 is not installed and the host compositor was not changed.
Local install/hash receipts live under `~/.cache/koett/main-install-3b4785f/`.
