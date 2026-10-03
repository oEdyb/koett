# Koett macOS overnight work — 2026-10-03

Branch: `codex/overnight-2026-10-03`, created from `feat/microphone-selection`.
The original uncommitted microphone work was committed unchanged first as `79adbfe`.
The installed app, signing/notarization, release ZIP, and Windows/Linux work were not changed. The initial overnight work remained local; Olle subsequently authorized pushing this branch after the microphone follow-up passes verification.

## Long-dictation measurements

The reproducible benchmark uses the production loader source via a source symlink, the pinned FluidAudio 0.15.6 revision `4dbf4f9f9a5ff3a53ade848d7ba4e3df13db859b`, Parakeet v2, default four chunk workers, and one loaded/prewarmed manager. Baseline is the original `manager.transcribe(URL, decoderState:)`; after is `DictationTranscriber.transcribe`.

Full-ASR replay results, including all six pairs for each length:

- **360 seconds:** median 84.088 → 74.360 s; mean 74.013 → 79.302 s; before range 24.135–116.013 s, after 22.873–157.842 s. After was faster in 4/6 pairs; median matched saving 2.664 s (median paired percentage 4.51%).
- **600 seconds:** median 67.912 → 40.248 s; mean 97.410 → 57.544 s; before range 12.848–224.027 s, after 26.468–146.462 s. After was faster in 4/6 pairs; median matched saving 27.664 s (median paired percentage 23.99%).
- **Accuracy:** every pair passed full transcript equality; all runs of each fixture share its exact text hash and confidence.

The Mac had severe, changing contention from unrelated work. Both medians are lower, but the six-minute mean is worse and ranges are extremely wide. These full-ASR results cannot establish a dependable improvement of that size or attribute inference-time differences to this change. The separate preparation measurement isolates the actual removed cost.

Preparation results, including all thirty alternating pairs per length:

- **360 seconds:** median 21.926 → 4.316 ms; mean 23.506 → 4.397 ms. The new loader won 30/30 pairs; median matched saving 17.571 ms (82.13%).
- **600 seconds:** median 33.021 → 6.118 ms; mean 32.631 → 6.498 ms. The new loader won 30/30 pairs; median matched saving 26.424 ms (78.25%).
- **Audio preservation:** all sixty pairs passed full sample-array and SHA-256 equality, with 5,760,000 and 9,600,000 frames respectively.

The dependable measured reduction is roughly 18–27 ms of long-recording preparation. Model inference dominates the total wait; the much larger full-ASR differences above remain uncertain. Complete preparation runs: [`2026-10-03-preparation.json`](../Benchmarks/LongDictation/2026-10-03-preparation.json).

Complete raw runs: [`2026-10-03-asr.json`](../Benchmarks/LongDictation/2026-10-03-asr.json). Hardware, fixture/source hashes, scope, protocol, limitations and computed statistics: [`2026-10-03-summary.json`](../Benchmarks/LongDictation/2026-10-03-summary.json).

Two fixtures contain 360 and 600 seconds of speech, created by concatenating/trimming the existing local `koett-wispr-stress-test.wav` recording. These are repeatable long-session replay fixtures, not two newly recorded microphone sessions. Six before/after pairs alternate execution order. All pairs are retained; any warm-up-excluded view is labeled separately. Every pair requires identical full transcript text; committed results contain only text SHA-256 hashes, confidence, duration and timings.

Timing starts with reading the stopped recording and ends with the final ASR result. It includes audio loading and all chunk inference/merging, excludes initial model loading, optional S1-mini formatting, the microphone stop, clipboard/Command-V, and history writes. Physical stop-to-visible-paste latency still needs device verification.

Hardware: Apple M5 MacBook Air, 10 cores, 24 GB; macOS 26.6.2 (25G83); Swift 6.4. The canonical WAV fast path is bounded at 15 minutes (57.6 MB of samples); longer or differently formatted inputs retain FluidAudio's disk-backed file path. Model, chunk size, overlap, decoder, default concurrency and formatting are unchanged.

Reproduce:

```sh
swift Benchmarks/LongDictation/build-fixtures.swift <16k-mono-speech.wav> .tmp/overnight-new-fixtures
swift build -c release --product LongDictationBenchmark
.build/release/LongDictationBenchmark .tmp/overnight-new-results.json .tmp/overnight-new-fixtures/dictation-360s.wav .tmp/overnight-new-fixtures/dictation-600s.wav
.build/release/LongDictationBenchmark --prepare-only .tmp/overnight-new-preparation.json .tmp/overnight-new-fixtures/dictation-360s.wav .tmp/overnight-new-fixtures/dictation-600s.wav
```

The preparation benchmark compares the pinned upstream conversion/write/synchronize/mapping stage against the production sample loader across thirty alternating pairs per fixture. Its equality checks and SHA-256 calculation run outside the timer; it requires complete arrays to match exactly. It isolates fixed preprocessing cost, rather than attributing differences in model inference to audio loading.

## Changes and regression coverage

- `Sources/Koett/DictationTranscriber.swift`, `DictationSession.swift`, `AssistantSession.swift`: bypass redundant conversion/temporary-file synchronization for bounded canonical WAVs, read every frame including legal short reads, and run loading off the main actor; `DictationTranscriberTests` covers exact samples, 301-second input, limits/formats, bad files and cancellation; paired real-model benchmark checks output equality and latency.
- `Sources/Koett/DictationSession.swift`: publish Nemotron capture ownership only after both recordings start, roll back failed startup, detach the live recorder on stop; `DictationLifecycleTests` covers backup startup failure, live startup rollback and capture release.
- `Sources/Koett/NemotronStreamingAdapter.swift`, `MicrophoneRecorder.swift`: dispose active Nemotron input/task on deallocation and permit device-free capture injection for regression tests; `testDroppingAnActiveLiveCaptureStopsInputAndProcessing` and the startup/stop tests cover lifetime.
- `Sources/Koett/MicrophoneInput.swift`: reject callback replacement after capture startup so a live HAL callback cannot reference freed state; `testCaptureCallbackCannotBeReplacedAfterStarting` covers the one-shot reservation used by the production start path.
- `Sources/Koett/TranscriptStore.swift`, `TranscriptRecord.swift`: give each completed result a stable ID and atomically replace history so retry after a partial/crashed append cannot duplicate it or discard the prior file; persistence tests cover exact metadata/text, crash-after-write, repeated IDs versus repeated dictations and invalid history bytes.
- `Sources/Koett/TranscriptFileLock.swift`, `TranscriptStore.swift`: protect prepare/read/deduplicate/replace/synchronize with a stable advisory sidecar lock; `testConcurrentIndependentStoresPreserveEveryTranscriptExactlyOnce` covers concurrent cooperating writers.
- `Sources/Koett/FailedTranscriptStore.swift`, `TranscriptPersistence.swift`: retain replayable recovery records and replay them exactly once after storage recovers or the app restarts, preserve malformed files and recover legacy Markdown intact; restart, corruption, legacy and partial recovery tests cover these paths.
- `Sources/Koett/TranscriptRecovery.swift`, `Koett.swift`: keep completed text and Last actions in memory after total disk failure, retry it later without replacing a newer Last transcript, and replay only in production startup; total-failure, recovery-only durability, last-write failure and newer-Last tests cover the boundary; injected stores isolate controller tests.
- `Sources/Koett/TranscriptRecovery.swift`, `TranscriptPersistence.swift`, `KoettMenu.swift`: expose all memory-only completed results through Copy Unsaved Transcripts while preserving the prior Last transcript after empty cleanup; `testAllUnsavedResultsRemainCopyableWithoutReplacingLastForEmptyCleanup` exercises the actual queued Copy action.
- `Sources/Koett/Koett.swift`: refresh menu state at every session state transition so Last actions do not remain disabled after dictation; `testMenuRefreshesWhenDictationCompletesSoRecoveryActionsEnable` checks actual menu items.
- `Sources/Koett/DictationDelivery.swift`, `DictationSession.swift`, `DictationFailurePresentation.swift`: post paste before persistence, persist once even after clipboard/paste failure, and keep simultaneous storage failure visible; `DictationDeliveryTests` covers ordering, timestamp and each failure combination.
- `Sources/Koett/MediaTranscriptPersistence.swift`, `MediaSession.swift`: recover a failed media history write with its text, title and source URL and present the warning; `MediaTranscriptPersistenceTests` covers fallback metadata and successful-media isolation.
- `Sources/Koett/TerminationProtection.swift`, `KoettApp.swift`, `RecordingOverlay.swift`: refuse ordinary Quit during active work or while a completed transcript is only in memory, retry storage before deciding, and keep recording feedback running; termination tests cover all busy states, volatile text recovery and live meter/timer tracking.
- `Sources/Koett/Koett.swift`, `DictationSession.swift`, `AssistantSession.swift`, `MediaSession.swift`: preflight safe termination before engine relaunch and block new sessions throughout the asynchronous restart window; volatile-relaunch refusal and `testRelaunchPreflightBlocksAllSessionAdmissionBeforeStartingReplacement` cover duplicate-instance prevention.
- `Sources/Koett/AssistantWorkLifecycle.swift`, `AssistantSession.swift`: cancel owned screenshot/work tasks, coalesce interruption/restart, and check cancellation around ASR, screenshots, cloud calls and speech; `AssistantLifecycleTests` covers ownership, each suspension, preserved completed question and successful ordering.
- `Sources/Koett/AssistantSession.swift`: stop prewarmed speech after aborted/short/failed questions and preserve Ask audio after failed local ASR; lifecycle tests distinguish ASR/empty-question failure from cloud failure and successful cleanup.
- `Sources/Koett/ScreenContextCapture.swift`: check cancellation before capture work and after expensive awaits; `testCancelledCaptureStopsBeforeUsingScreenCaptureKit` covers cancelled entry and pipeline tests cover cancelled screenshot completion.
- `Sources/Koett/AssistantSpeechConnection.swift`, `CartesiaSpeechOutput.swift`: reject stale send/receive/playback completions by connection generation, weakly own the receive loop, and stop socket/audio resources after the final played buffer; `AssistantSpeechConnectionTests` covers interruption, old callbacks, send failures, cancellation and drain ordering.
- `Sources/Koett/AssistantClient.swift`, `AssistantConfiguration.swift`: require an HTTPS host, reject empty/truncated streams, stop cancelled buffered events and cancel the underlying HTTP transfer after DONE/error; `AssistantStreamTests` includes a real URLSession/URLProtocol transfer-cancellation test without external requests.
- `Sources/Koett/AssistantPanel.swift`: preserve partial answer and Copy after stream failure, show storage/response errors separately and avoid claiming a screen was included after capture failure; `AssistantPanelStateTests` covers state, copy availability and reset.
- `Sources/LongDictationBenchmark/`, `Benchmarks/LongDictation/`, `Package.swift`: add reproducible alternating long-audio and preparation benchmarks using the same production loader source; they assert complete transcript and sample equality rather than relying on summary hashes alone.

## Microphone availability follow-up

- `Sources/Koett/MicrophoneSelection.swift`: exclude unusable Core Audio inputs, identify built-in devices, and replace a missing explicit UID with built-in/default/first available input; keep available choices and System Default stable, and preserve the preference while no input exists; `MicrophoneAvailabilityTests` covers every fallback and preference case.
- `Sources/Koett/MicrophoneAvailability.swift`: reconcile and save available choices at startup and on changes, stop dictation/Ask after their capture device disappears, retain captured audio and cancel pending Ask work; disconnect tests check exact retained frame counts, stopped input and cancelled screenshot work.
- `Sources/Koett/MicrophoneAvailabilityMonitor.swift`: monitor device list, default input, current-input alive state, wake and activation with rollback, matching removal and stale-callback guards; nine injected-registration tests cover setup, retargeting, queued callbacks, failure/retry, notifications and deallocation.
- `Sources/Koett/Koett.swift`, `DictationSession.swift`: start monitoring before setup and keep the alive listener on the actual capture device, then retarget after recording stops; `testAliveListenerFollowsDefaultAfterRecordingStopsAndTheNextOneStarts` covers the old-listener regression.
- `Sources/Koett/MicrophoneRecorder.swift`, `NemotronStreamingAdapter.swift`, `DictationSession.swift`: mark both live and backup capture unavailable after disconnect and retain even failed recordings shorter than 0.3 seconds; short/long Nemotron disconnect tests check every retained WAV frame and stopped ownership.
- `Sources/Koett/MicrophoneMenu.swift`: display the repaired available input or **None available**, persist choices in the controller's defaults, and resolve again before capture; menu and no-input recording tests cover visible state and failure.
- `Tests/KoettTests/MicrophoneAvailabilityTests.swift`, `MicrophoneAvailabilityMonitorTests.swift`, `MicrophoneTests.swift`: add 23 hardware-free regressions and update the prior missing-device expectation for automatic fallback; no live microphone or cloud service is used.
- `docs/feature-contracts.md`, `docs/microphone-selection.md`, `docs/overnight-2026-10-03.md`: record fallback persistence, observer/capture behavior, historical versus current verification, and real-device limits.

The fallback becomes the saved explicit choice; reconnecting an old microphone does not take it over. An interrupted session retains its old-source audio and uses the replacement on the next recording. Koett does not combine audio from different devices mid-session or change the macOS default microphone.

## Verification

- `swift test`: **167 tests passed, zero failures**, final full run after the microphone follow-up on 2026-10-03 (4.3 s suite time).
- `swift build -c release`: passed, final microphone-follow-up rerun 43 s wall time.
- `git diff --check`, `zsh -n install-macos.sh`, `plutil -lint Packaging/Info.plist`, `plutil -lint Packaging/Koett.entitlements`: all passed.
- Full-real-model and preparation benchmarks: exit zero; twelve full-transcript pairs and sixty sample-array/hash pairs passed equality checks.
- Independent fresh-context review: zero unresolved findings. Constructor-replay side effects, recording-feedback quit regression, partial-answer overwrite and duplicate-instance relaunch were found, fixed and rechecked.
- Microphone follow-up review: zero unresolved findings after fixing a stale capture-device listener, short failed Nemotron audio deletion, and preference changes during a temporarily empty device list. All three fixes have regression coverage.
- HTTP cleanup test checks `URLProtocol.stopLoading` directly and keeps the client alive through its assertion; its control proves the unfinished fixture stays open while the iterator is retained. Foundation iterator teardown also stopped the transfer without explicit cancellation, so the explicit cancel is defensive cleanup rather than a reproduced baseline leak.
- Changed-file credential-pattern scan: no matches. No API keys, audio fixtures, private transcript text or model binaries were added to the tracked receipts.

## Needs a real device

- Fresh 5+ minute built-in/USB/AirPods recordings, actual stop-to-visible-paste timing, target-app focus and Command-V delivery.
- Microphone unplug/reconnect and format changes, plus a real Nemotron session and backup recovery after stream failure.
- Live permission/capture cancellation, rapidly interrupted Ask, Cartesia output and final-buffer drain on actual hardware/network.
- Warning pill, persistent menu, partial-answer error presentation, Copy/Paste Last and startup recovery in a newly installed build.
- VoiceOver, Reduce Motion and sleep/wake interaction; existing accessibility regression tests continue to run.

No install or real interactive microphone/cloud session was performed: the installed app was explicitly protected.

## Not fixed and why

- Recordings above 15 minutes retain disk-backed ASR: the fast-path memory cap avoids unbounded allocations; model/chunk strategy changes need separate accuracy evidence.
- Optional S1-mini's latency and meaning changes were not changed: raw text remains default, and model/cleanup upgrades were outside this measured ASR fix.
- A forced kill, power loss or system shutdown after *every* disk destination fails can still destroy memory-only text; the app now makes this visible, keeps Copy/Paste Last and Copy Unsaved Transcripts, retries, and blocks ordinary Quit. Disk durability cannot be created on an unwritable disk.
- Arbitrary external editors and the old installed binary do not obey the new advisory history lock; cooperating updated stores are covered, and the installed app was not changed.
- Vimeo's known anonymous OAuth 401, real rich Markdown/math panel confirmation, and Windows/Linux/release gates remain as documented in AGENTS.md; external access/platform/release work was outside this macOS task.
