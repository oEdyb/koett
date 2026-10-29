# Linux parity inventory — model and formatting increments, 2026-10-29

Scope: local model management and compact native controls. This is **not** a
completed Mac port. No installation, live preference change, commit or push.

Optional spoken Ask source and private protocol/playback evidence are described
in [the speech increment](linux-spoken-ask.md). Cartesia live compatibility remains unverified.

## Source inventory and remaining contract

- `Sources/Koett/KoettMenu.swift`: ordered History, setup/retry/errors, explicit
  engine choices, microphone, Formatting, toggle/hold, shortcut binding/reset,
  Ask submenu, media action, latest outcome, Copy/Paste Last, unsaved text,
  failed recordings/transcripts, version and Quit. Linux retains History,
  recording, settings, recovery Copy and Quit; adds Local models. Direct
  microphone/mode/shortcut menu actions, Paste Last, setup/permission routing,
  version and complete recovery-folder menu parity remain pending.
- `FormattingPopover.swift`: 300×190 transient popover, 16px inset, 15pt title,
  8px grid row spacing, 12px columns, trailing labels, 160px controls. Output
  Raw/S1-mini, four styles, Prose/Lists and General/Email are implemented in the
  GTK persistent popover and menu action. Source-derived inset/grid/control widths
  guide the compact surface; exact Mac screenshot fidelity remains unverified.
  Native GTK currently uses an Output chooser rather than AppKit segmented control.
- `RecordingOverlay.swift`: 236×48 capture pill, 250×48 result pill, waveform,
  phase/progress, timer, cancel and outcome/recovery. Existing Linux pill stays;
  full Ask/media phases and comprehensive exact geometry parity remain pending.
- `AssistantPanel.swift`: 480px floating side panel, bounded 440–660px height,
  question, screen-included marker, streaming rich answer, listening/transcribing/
  answering/completed/error states, Copy and close. Linux now has voice-question
  capture, shared question persistence, portal one-frame JPEG, provider settings,
  Secret Service, cancellable HTTPS SSE and a lazy ephemeral WebKitGTK rich
  renderer using embedded pinned Mac assets. Full/partial exact Copy, real ASR
  cancellation/no-provider-request, permission recovery and subsequent dictation
  passed integrated private-CA acceptance. X11 single-frame memory capture passed
  on owned rootful Xwayland. Capable Wayland uses a 480px right-side layer-shell
  panel; unsupported desktops explicitly fall back to a normal window. Physical
  multi-monitor targeting, complete screen-reader acceptance, distribution
  packaging and appearance approval remain open.
  See [Ask increment and exact proof boundaries](linux-ask-increment.md).
- `HistoryWindow.swift`: 960×680 initial window, 360×420 minimum, 12px search
  inset, 300px list, narrow navigation below 680px, exact selectable text,
  Copy/Export and failure alerts. Existing Linux SQLite/search/Copy/Export and
  responsive split remain. Header and settings density reduced this increment.
- `feature-contracts.md`: media shortcut, one focused public media item, bounded
  subprocess download/conversion, cancellation and dated result/source storage
  remain pending. Speak Replies, voices, Cartesia key and interrupted output
  remain pending. Experimental Nemotron streaming is implemented and has real native Linux
  inference/capture evidence; see the Nemotron increment below. Apple-only
  support and complete recovery fault-injection acceptance remain open.

## Model increment

110M remains default. `selected_model` is explicit and backward compatible with
existing preferences; custom model-directory behavior remains available internally.
Local models is reachable through the real GTK menu. Both choices verify pinned
archive and extracted-file SHA-256 before loading. The v2 manifest records the
official sherpa release asset 283097678, NVIDIA attribution and CC BY 4.0 license.
VAD uses the existing separately pinned Silero artifact. A cached trial archive
can be reused after full digest verification; it is never trusted by filename.

A model switch is refused outside Ready. The previous transcription worker stays
alive while the candidate is prepared and loaded. Only successful load plus saved
preferences replaces it. Failure/cancel leaves the worker and preference unchanged.
Selecting the same model reloads it. Restart loads the saved choice. Failed optional
startup falls back to 110M with a notice. Installed models use separate directories;
no transcript schema, native vendor build or Mac source changed.

Settings uses a compact category selector and denser labeled rows rather than the
rejected wide tab strip. The intent is the Mac's compact menu/grid control language;
visual approval remains a parent/user gate. GTK is retained; no dependencies added.
Future features should add live menu actions and small focused surfaces, not dead
buttons or a dashboard of future features.

## Primary-source investigation

- https://catalog.ngc.nvidia.com/orgs/nvidia/-/collections/parakeet-tdt-0.6b/
- https://docs.nvidia.com/nim/speech/latest/reference/support-matrix/asr.html
- https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html

NVIDIA publishes v2 English and v3 supporting 25 European languages. Existence
of v3 does not establish a verified compatible local artifact. This increment
exposes only the pinned 110M and separately evaluated official v2 conversion;
v3 remains pending conversion/artifact/native-inference validation.

## Acceptance and evidence

Expected files: Rust model/settings/runtime/Linux UI, Windows exhaustive update,
model manifest, native harness and these milestone documents. Gates and selected
native captures live under `CrossPlatform/evidence/parity-models-20261029/`.
Full generated fixtures are under `/tmp/koett-parity-*`. These are synthetic/public
fixtures; no live transcript/audio/screen data or keys are accessed.

Acceptance: explicit model choice without JSON editing; distinct active identity
and inference; restart persistence; failure/cancel fallback; busy refusal; exact
history/search/Copy/Export; native menu/settings/models light/dark/narrow/enlarged
font inspection. Evidence status is recorded separately from implementation claims.

### Verified local milestone

Fresh Rust gates: 96 passing tests / 3 existing ignored, strict all-target clippy,
format, locked release, and the requested Windows source check. Native public/
synthetic fixtures prove explicit v2 selection, distinct inference, cancel,
invalid-cache fallback, recording refusal, persistent restart and Settings merge.
Final light and dark/Sans-14 visual passes prove exact Copy/Export/search, named
keyboard menu, settings category navigation, narrow models/history and empty state.
See the evidence README for exact commands, screenshot generations and limits.

The category selector has an explicit native accessibility name, distinct from the
microphone selector. Existing native UI/menu/visual harnesses now route category
selection through that control instead of removed tab buttons.

### Parent review follow-up

Fresh code/A and code/B reviews found corrupt optional caches preventing network
recovery, startup Cancel using the switch-only token, and restart evidence checking
Ready without active identity. Parent fixed cache source fallback with cancellation
and regression coverage, unified startup/switch preparation cancellation, and
strengthened restart identity assertions. The native failure fixture now rejects a
preference save rather than assuming corrupt-cache recovery should fail. These
updated native assertions passed in the private formatting lab: read-only preference
failure retains compact, and reopening Models after restart proves active v2.
No installation has occurred.


## Formatting increment

`CrossPlatform/src/formatting.rs` owns verified official S1-mini Q4_K_M setup,
loopback authenticated llama-server requests, token limits, bounded readiness and
inference, and owned-process teardown. The exact Mac model revision/bytes/SHA-256
remain pinned. No dependencies were added. llama-server is an explicit external
prerequisite on PATH; evaluation used official CPU-only b11981, isolated under
`/tmp/koett-format-eval`, not installed. Runtime package distribution is still a
release/packaging concern.

Raw is default. Formatting controls persist independently of ASR model selection.
Setup runs on a separate worker while capture remains available. Controls refuse
changes during capture/transcription or another formatting setup. A per-child
credential avoids accepting unrelated services after the ephemeral port reservation
is released. The owning preparation thread remains alive while its server is warm;
Linux parent-death binding and explicit kill/wait cover forced and normal exits.
Raw disables and releases the server. Startup retries enabled optional formatting;
failed setup or inference leaves dictation available as raw.

Successful cleanup sends final text through existing delivery and saves original
ASR text in the existing raw history field, exactly once. Missing/empty/malformed/
truncated output, process failure and request errors fall back to raw. Existing
replacements remain applied by the delivery adapter. No cloud path, Swift source,
selected ASR default, history schema or vendor source was changed.

Evidence: `CrossPlatform/evidence/parity-formatting-20261029/`. Native captures
prove Linux appearance and behavior, not Mac screenshot parity or user approval.
Real CPU inference uses synthetic utterances and the model's public ASR sample.
Meaning checks are observations; optional rewriting is not established meaning-safe.
Ask, media, TTS, expanded menu/recovery/permissions/shortcut parity, experimental
engines, packaging/installation, physical-seat/compositor acceptance and final
visual review remain pending as inventoried above. Searchable History is retained
as an explicitly requested Linux feature, not described as current Mac main behavior.

### Formatting parent review follow-up

Two fresh held-out reviews found three issues: loopback requests inherited proxy
settings, Settings Save could overwrite formatting completed after the dialog
opened, and failed disable could display Raw while the cleaner remained enabled.
Parent explicitly disables proxies for local requests (download proxies retained),
merges current formatting in Settings Save, and sends a formatting-state refresh
and specific error after every completed preference edit. A subprocess regression
with forced HTTP/ALL proxies proves local authenticated requests reach only the
loopback fixture. 104 Rust tests and strict Clippy pass after these changes.
The two updated native settings-failure/race journeys need fresh private proof;
prior screenshots/receipts predate these fixes. Installation remains unchanged.

## Post-review formatting proof before media integration

Both formatting review fixes passed real private-lab journeys: stale Settings
Save retained enabled S1-mini and the same warm server; read-only preferences
restored the S1-mini selection, showed the save error, and the next dictation
actually produced raw plus cleaned text. Restoring write access let Raw persist
and stopped the cleaner. Evidence: `CrossPlatform/evidence/parity-media-20261029/`.
That prerequisite-only run did not implement media. The media operation increment
below supersedes its implementation status; the original receipt remains historical.

## Media operation increment — 2026-10-29 (evidence label 20261030)

DONE: manual HTTP(S) URL transcription through GTK menu/tray and
`koett media URL`; `koett cancel-media` cancels the same runtime operation.
GTK has URL entry, Enter/Escape, phase feedback, scrollable exact results and
Copy. Media completion never invokes dictation cleanup/replacements or paste.
The existing selected/warm transcription worker accepts asynchronous bounded
PCM-file requests. Decode uses 30-second buffers, with cancellation between
buffers; it does not allocate three hours of floating-point audio.

`src/media_transcription.rs` owns temporary-directory cleanup, yt-dlp/FFmpeg
process groups, socket/overall timeouts, source/decoded-size checks, URL
validation and separate media Markdown storage. Source, exact Unicode title,
model and exact text persist together. Primary failure uses the shared failed
transcript writer without changing the history schema. Both-destination failure
keeps Copy available and reports the storage error. `runtime.rs` reserves the
session until the fetch/ASR reply ends; capture and configuration cannot overlap
media ASR. `linux/media_view.rs`, `linux/control.rs` and the existing overlay
provide the native/CLI entry points. URL parsing and per-child file-size limits add exact direct pins to the
already-locked url 2.5.8 and libc 0.2.189 dependencies; Swift, vendor and the default 110M/Raw stay
unchanged.

VERIFIED: 108 Rust tests pass, three existing ignores; fmt, strict locked
all-target Clippy, locked release, Rust 1.92 check, Windows source check with
DOCS_RS/pkg-config overrides, Ruff 0.17.2 and Python compilation pass. Real
installed yt-dlp + FFmpeg + the warmed 110M worker transcribed the public model
sample through a loopback HTTP fixture after cancellation. The probe verified
exact text, Unicode metadata and forced primary-save fallback. A controlled
HTTP redirect to an owned local fixture was rejected. Synthetic child success,
nonzero exit, timeout/cancellation and temporary cleanup have deterministic
tests. Private `koett-media-operation` GTK journeys verified exact Copy against
the saved result, keyboard URL/Enter/Escape/Copy, narrow results, errors and
microphone capture after cancellation. Light/dark native captures were inspected.
Evidence: `CrossPlatform/evidence/parity-media-operation-20261030/`; see its
receipt for selected captures and exact limits. The lab and owned processes
were stopped. No installation, live preferences, commits or pushes.

REMAINING: automatic focused-browser URL detection; configurable global media
shortcut (Ctrl-Shift-T currently works inside the GTK application); Open Media
Transcripts folder action; compact Mac-style integrated preview/Copy pill and
appearance approval. The current result uses the existing recovery pill/button
plus a native result window. ASR cancellation waits for the current bounded
30-second audio decode; it does not terminate the warm recognizer. Three-hour
recognition accuracy and full 2 GiB network stress are not experimentally proven.
Only direct HTTP(S) media formats are selected: manifest-only HLS/DASH and
login/DRM/cookie-dependent sources fail explicitly. No arbitrary public-site
compatibility promise or live protected-site test. Recording after cancellation
used isolated silent input; no physical microphone or host clipboard was used.
Parent owns independent reviews, remaining parity, installation and publication.

### Media parent review follow-up

Fresh reviews found the media window sharing ordinary dictation result state,
a destructive storage probe not enforcing isolated XDG data, and a recording
regression that asserted dispatch rather than actual capture. Parent added a
separate media result cache (cleared on native submission), auto-isolated the
probe in a fresh private temporary child environment, and strengthened the
harness to assert Recording, Ready and a public-fixture transcript. 108 tests
pass and strict Clippy is clean; updated native journeys still need a rerun.
The original receipt predates these fixes. No installation has occurred.

Page URL support depends on yt-dlp extracting a direct HTTP(S) media stream;
this is broader than only URLs naming media files. Manifest-only HLS/DASH remains
excluded by the protocol selector, and automatic focused-tab discovery is pending.

Ask implementation research: the official ScreenCast portal returns a restricted
PipeWire FD and supports source restore tokens; use a one-frame in-memory capture,
not portal Screenshot files. Reference:
https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html
Linux keys belong in Secret Service, not JSON:
https://specifications.freedesktop.org/secret-service/latest-single/
Host has GStreamer/app 1.28.8, PipeWire 1.6.10, libsecret 0.21.8.2 and secret-tool;
webkitgtk-6.0 pkg-config is absent. Rich Ask now uses an optional stable C ABI
loader and was tested with a signature-verified isolated official runtime package,
without changing system packages or the GTK core floor. See the Ask evidence
receipt for runtime prerequisites and exact remaining release checks.


## Nemotron streaming increment

Experimental Nemotron English 0.6B / 560 ms is selectable and persistent. It uses
one warm CPU online recognizer, fresh per-recording state and bounded capture
submission; Parakeet v2 stays warm for ASR recovery and offline Ask/media. Failed
ASR retains a collision-safe private WAV and exposes Failed recordings. See
[implementation, source/license boundaries, exact checks and remaining acceptance](../CrossPlatform/evidence/parity-nemotron-20261030/README.md).

Implemented/proven: verified official archive/per-file manifest, actual native
inference, repeated CPAL captures, once-only history, no success WAV, long/short/
silence state reset, injected streaming/finalization fallback, failed-worker WAV
recovery, explicit active identity and restart. Light/dark/narrow inspection is
engineering evidence; appearance approval remains open.

Full parity still requires main UI/menu ordering and direct actions, complete
shortcut behavior, physical microphone fallback/hotplug, media HLS/browser
detection, distribution packaging, unresolved upstream model redistribution
notices, independent final review and the remaining integrated recovery tests.
No installation, default-model change, live preference edit, commit or push.

### Nemotron parent review

Code/A found model preparation could reset cancellation after shutdown requested
it. Parent moves startup reset before the worker starts, checks shutdown before
initial preparation, and brackets model-switch reset with shutdown checks before
publishing setup. The regression asserts queued setup cannot clear shutdown.
Focused runtime tests and strict Clippy pass after the fix.

Code/B requested retaining WAVs only after two recognizer failures, including for
standalone Parakeet and Ask. That restriction is rejected: the canonical core
contract (feature-contracts.md, Core dictation) retains audio after any exhausted
local ASR failure. Nemotron has two attempts; standalone Parakeet has one. A
capture-overflow result is incomplete even if its decoder returns text, so warm
Parakeet recovery remains required before trusting it. Successful ASR still never
retains audio merely because formatting, delivery or transcript storage fails.
