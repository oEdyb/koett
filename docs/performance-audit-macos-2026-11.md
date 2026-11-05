# Mac performance audit — November 2026

Status: all 24 repeated native M5 controlled-input Stop-to-target receipts passed
independent freshness, reference-length and tail validation. Native dictionary
latency, corpus recall, vocabulary-switch memory and eight guest validations passed. The installed host app,
preferences and History were not changed. Host target focus and clipboard were
used in the private suite; clipboard backups stay in memory and are restored after
each accepted receipt. All committed transcript payloads are public synthetic fixtures.

## Scope and source

M5 MacBook Air, macOS 26.6.2 (Darwin 25.6.0), native arm64 release tests. Source
`c0343ae` before the default-disable change. FluidAudio remains exactly 0.15.6,
revision `4dbf4f9f9a5ff3a53ade848d7ba4e3df13db859b`. No dependency upgrade.

[All raw receipts and summaries](../Benchmarks/Dictionary/performance-20261105/)
are committed alongside [the standalone target/driver sources](../Benchmarks/Dictionary/PasteAudit/).

The original source establishes that this is an additional encoder pass, not
reuse of the main 0.6B TDT encoder:

- [`VocabularyBoostingSession.rescore`](https://github.com/FluidInference/FluidAudio/blob/4dbf4f9f9a5ff3a53ade848d7ba4e3df13db859b/Sources/FluidAudio/ASR/Parakeet/SlidingWindow/CustomVocabulary/VocabularyBoostingSession.swift#L78)
  calls `spotKeywordsWithLogProbs` with raw `audioSamples`.
- [`CtcKeywordSpotter+Inference`](https://github.com/FluidInference/FluidAudio/blob/4dbf4f9f9a5ff3a53ade848d7ba4e3df13db859b/Sources/FluidAudio/ASR/Parakeet/SlidingWindow/CustomVocabulary/WordSpotting/CtcKeywordSpotter%2BInference.swift#L12)
  runs mel extraction and the CTC encoder; long input is split into overlapping
  chunks and each chunk calls `computeWithStagedModels` (encoder prediction at line 178).
- Koett's `DictationTranscriber` first completes `manager.transcribe`, then calls
  `boost` with text, timings and the same trimmed samples. Its booster loads the
  separate CTC 110M models. The 2 s deadline drops late results; it does not cancel
  the underlying inference. This is not the Linux two-full-0.6B-stream mechanism.

Newer FluidAudio/sherpa releases were reported during this audit; their behavior
is not attributed to this pinned build. Shared CTC-head support for a different
110M primary model does not establish encoder reuse for the default 0.6B model.

## Native paired dictionary latency

Same public synthetic Andrew voice corpus concatenation, exact 5/30/120/600 s
prefixes, mono PCM16 at 16 kHz. See `Benchmarks/Dictionary/prepare-latency.py` and
`performance-20261105/` for raw results, references and every repetition.
The main model, VAD and CTC session were warm. Three repetitions per length and
mode; order alternated off/spelling/CTC then CTC/spelling/off. A test-only wrapper
waited for any late CTC inference to finish before starting the next timed trial.

| Audio | Dictionary off median | Spelling only median | Change | CTC + spelling median | Change |
|---|---:|---:|---:|---:|---:|
| 5 s | 75.409 ms | 76.300 ms | +1.18% | 211.732 ms | +180.78% |
| 30 s | 184.457 ms | 185.113 ms | +0.36% | 706.763 ms | +283.16% |
| 120 s | 477.082 ms | 486.154 ms | +1.90% | 2334.432 ms | +389.31% |
| 600 s | 2055.412 ms | 2072.529 ms | +0.83% | 4056.221 ms | +97.34% |

This measures the real `DictationTranscriber` (file read, VAD, ASR and optional
boost) plus `DictionarySpelling`, **not stop-to-paste**. It excludes recording stop,
optional cleanup, clipboard, target checks, event posting and target insertion.
CTC retains the production 2 s deadline: at 600 s it times out and returns the same
text as spelling-only after adding approximately two seconds.

### Post-change default-path confirmation

The actual new default booster callback (disabled, with the same dictionary) plus
spelling was measured against no callback and no spelling. Three alternating
repetitions per length, same warm model/VAD and exact fixtures:

| Audio | Off median | New default median | Observed change |
|---|---:|---:|---:|
| 5 s | 76.109 ms | 75.786 ms | −0.42% |
| 30 s | 201.009 ms | 198.148 ms | −1.42% |
| 120 s | 555.509 ms | 563.526 ms | +1.44% |
| 600 s | 2464.218 ms | 2365.109 ms | −4.02% |

This original confirmation process loaded and warmed CTC even for the
default/off-only comparison. It is a fair paired comparison but not the normal
production loading state. A fresh process without CTC loading is reported below.

All length medians remain below the 10% budget. One individual 120 s pair was
+24.80%; the other two were −6.62% and +1.44%. This does not establish a
per-recording 10% ceiling. Negative differences are run variation,
not evidence that spelling speeds up ASR. All 12 default transcripts exactly
matched the first sweep's spelling-only transcripts. Raw repeats and equality
checks are in `default-latency.json` and `default-summary.json`.

### Fresh default/off process without CTC loading

After the host capture suite, the harness was changed to load/warm CTC only when
`ctc` is a requested mode. A new release process repeated all four lengths with
three alternating off/default pairs and no CTC session loading or warm-up:

| Audio | Off median | Default median | Change |
|---|---:|---:|---:|
| 5 s | 80.477 ms | 81.529 ms | +1.31% |
| 30 s | 205.861 ms | 190.362 ms | -7.53% |
| 120 s | 544.411 ms | 534.448 ms | -1.83% |
| 600 s | 2366.278 ms | 2372.256 ms | +0.25% |

All 12 default outputs exactly matched the earlier cheap path. This addresses
normal production loading state while retaining the original fair paired results.
The run passed in 19.765 s. A nonfatal Core ML `E5RT` zero-shape diagnostic was
printed; its origin was not localized. All 24 timed calls completed, no late CTC
work ran, and output equality checks passed. Raw output and every pair are in
`no-ctc-default-*`. Negative changes are timing variation, not a speedup claim.

A separate three-repeat 600 s probe explicitly measured late CTC work. Visible
pipeline times were 4453.781, 4516.367 and 4374.282 ms. After those returns, the
CTC tasks still ran for **6973.435, 6282.961 and 5986.941 ms** before completing.
These are external drain waits, excluded from visible latency. All three late
results were discarded by the production deadline. See `late-ctc.json`.

Term occurrences recovered identically across the three repetitions:

| Audio | Expected completed-span terms | Off | Spelling only | CTC + spelling |
|---|---:|---:|---:|---:|
| 5 s | 1 | 0 | 0 | 0 |
| 30 s | 9 | 3 | 4 | 7 |
| 120 s | 51 | 20 | 22 | 36 |
| 600 s | 254 | 95 | 103 | 103 |

These long-fixture counts cap each case-sensitive term's output occurrences at
its reference count. They exclude the unfinished final sentence's expected terms;
they are not word-aligned recall or evidence of human microphone accuracy.

Other build, model and VM jobs were held for this slot.
The benchmark used an exclusive `/tmp/koett-mac-benchmark.lock` file lock. All
Tart guests were stopped during native timing. Ordinary desktop processes remained active;
three repetitions and this fixture set are a measured result, not a universal
worst-case guarantee. Initial exploratory numbers with undrained timed-out work
were discarded.

## Fresh 210-clip recall recheck

The existing production-path corpus benchmark was rerun in release mode with
all three public synthetic voices and the 30 corpus terms (no sounds-like forms).
It passed in 50.553 s. Its explicit experimental CTC timeout is 10 s, which never
bound these short clips. Native plain median was 64 ms, CTC median 166 ms,
maximum 299 ms. These corpus passes run modes in blocks, so use the paired
length sweep above for the controlled latency comparison.

| Method | Correct terms / 162 | Recall | False terms | Positive WER | Negative WER | Broken negatives | Lost neighboring words |
|---|---:|---:|---:|---:|---:|---:|---:|
| Off | 49 | 30.25% | 0 | 14.43% | 0.28% | 0 | 0 |
| Spelling only | 61 | 37.65% | 0 | 12.35% | 0.28% | 0 | 0 |
| Experimental CTC + spelling | 117 | 72.22% | 0 | 5.80% | 0.28% | 0 | 0 |

Turning CTC off loses 56/162 correct term occurrences, **34.57 percentage points**
of recall on this corpus. Spelling still adds 12/162 versus no dictionary.
Raw synthetic transcripts and score receipts are in `performance-20261105/`.
No private dictionary, transcript history or microphone recording was read.

### Additional 120 s variability check

Nine extra paired runs had whole-pipeline differences from −25.69% to +14.10%
(median −0.79%). The timed disabled callback itself took 0.0011–0.0020 ms;
spelling took 0.612–1.431 ms. Combined correction cost was **0.613–1.433 ms**.
All nine default outputs matched the original spelling-only output. The whole
ASR timing variation is therefore reported separately from measured correction
cost; no per-recording latency ceiling is claimed. Raw data: `variance-*.json`.

## Empty/populated vocabulary switches

A separate native release process loaded/warmed the primary model, then tested
`empty → 20 terms → empty → 20 → empty → 20 → empty → 20 → empty` with both
constructors. Each update reached off/ready, settled for 500 ms and processed the
same five-second fixture through the production transcriber plus spelling.
All 18 steps passed. These are four reload cycles, not a long-duration leak test.

| Constructor | Baseline/default RSS | Loaded CTC RSS, first → fourth | Empty RSS after load, first → fourth |
|---|---:|---:|---:|
| New default, acoustic off | 98.797–99.078 MiB | Not loaded | Not loaded |
| Explicit experimental CTC | 99.031 MiB before load | 111.125 → 111.812 MiB | 106.391 → 107.031 MiB |

The experimental loaded readings increased 0.6875 MiB across four loads; empty
readings increased 0.640625 MiB across four unloads. Per-process physical footprint
was 28.235–28.313 MiB for the default, 32.516–32.548 MiB when CTC was loaded,
and 28.329–28.423 MiB after unloading it. These task counters do not measure total
model storage, accelerator allocations or all system-memory use.

Observed update-to-ready times for the four populated CTC steps were 333.633,
273.821, 276.140 and 267.971 ms (50 ms loading-status polling). Direct update calls
across all steps took 0.000334–0.039167 ms. Source `CtcModelCache.session` creates a
session with its cached models; `clear()` releases that cache task on empty updates.
The new default returns before loading any CTC model. Full before/after memory,
peak RSS, physical footprint, readiness and pipeline timings are retained in
`vocabulary-switches.json`; `DictionarySwitchMemoryTests` is opt-in.

## Decision

Disable acoustic boosting by default; keep spelling, sounds-like forms and exact
replacements. Only explicit experimental/benchmark callers may enable CTC.
The tested cheap path's length medians meet the 10% latency budget. It does not
preserve CTC's term recall at 30 and 120 s; this is a speed/recall
tradeoff. Both original requirements were not achieved.

## Native M5 controlled-input Stop → target — before cache fix

The host comparison uses source builds of public v0.1.1 `c72f834` and current
`85dde51`, both with the same pinned FluidAudio, raw formatting and empty
dictionaries. The model and a complete five-second pipeline are warmed before
each measured group. Three groups per build alternate old/current, current/old,
old/current. All 24 measured captures and six warm-ups completed. The independent
verifier checks 24 unique requests and source generations, six distinct source
processes, warm-up generation 1 and measured generations 2/3/4/5 in every group.
Clipboard representations were restored after every accepted capture and at exit.

| Audio | v0.1.1 median (range) | Current median (range) | Median change |
|---|---:|---:|---:|
| 5 s | 148.342 ms (148.190–182.756) | 157.781 ms (149.541–216.731) | +6.36% |
| 30 s | 258.610 ms (253.191–260.984) | 298.920 ms (297.300–368.482) | +15.59% |
| 120 s | 521.806 ms (514.264–534.283) | 652.968 ms (652.855–710.876) | +25.14% |
| 600 s | 2134.911 ms (2114.224–2150.451) | 2581.464 ms (2543.667–2720.389) | +20.92% |

Current medians were 6.36%, 15.59%, 25.14% and 20.92% higher. At 30–600 s all
three current observations exceeded all three old observations in this suite.
This supports a slower **instrumented controlled-input pipeline** observation,
not a cause diagnosis or an exact shipping microphone comparison. The five-second
ranges overlap and include old-only tap teardown bias; no faster claim is made.
The last current group was slower than its earlier groups, with no power/thermal
telemetry to attribute the variation.

All three payload lengths were identical within each build/length: old/current
83/83, 472/472, 1837/1841 and 9157/9154 characters. Final text was checked against
the marked fixture cutoff, not just the initial 8000-character threshold. Source
`*-reference.json` spans were shifted by the recorded marker-prefix duration:
0/8/37/191 complete corpus clips at 5/30/120/600 s, with partial final phrases
reaching “without the…”, “booking app”, “both” and “Jacob and”. The reference-complete
text lengths are 35/436/1828/9221 characters including the marker. Accepted output
must retain at least 97% of that length, contain the final phrases, and differ by
at most 1% in length across builds/repeats. These are completeness guards with
known ASR errors, not proof of word-for-word reference accuracy. The full output,
all timings and per-request reference checks are in `native-paste.json` and
`native-summary.json`. UUIDs are unique bookkeeping; source/event generations
supply independent freshness.

Original same-invocation latency logs are retained in
`native-same-invocation-stages.json`, including six warm-ups. They report production
Stop-to-ASR-start, ASR/frontend, text preparation, delivery and paste-post stages,
paired by request with the measured receiver timestamp. No stage sums substitute
for the measured Stop-to-insertion endpoint. Original production console logs
and collector cleanup are preserved in `native-console/`, with SHA-256 hashes
in provenance. The same nonfatal E5RT zero-shape diagnostic appears during model
startup in these logs; all 30 warm/measured source invocations completed.

The input seam replaces PCM immediately before recorder file writes. Old uses a
real input `AudioQueueProcessingTap`; current replaces its live input buffer before
`MicrophoneFileSink.append`. The real recorder, actual Stop/close, production
transcription/preparation, paste-readiness checks, clipboard and Command-V remain
in place. The source Stop methods are not replaced. No separately measured
components are added together.

The start timestamp is immediately before the real controller Stop call; the
end timestamp follows `super.paste` in a separate native `NSTextView` process. Its
window is alpha-zero. This is **native M5 controlled-input pipeline latency**,
excluding microphone acquisition, room acoustics, physical Stop-key dispatch and
compositor rendering. The two recorders retain their real input/file rates: old
16 kHz, current 48 kHz. The common generator linearly resamples the same frozen
16 kHz PCM where required; normal post-Stop conversion remains part of the timer.

Each capture loads and verifies the fixture SHA, resets a source capture
generation, injects the complete fixture on the live input clock and requires
its spoken marker in the actual target. Source-tagged clipboard generation,
source PID, unrelated pre-capture sentinel, request UUID, injected frame count,
minimum payload length and exact receiver/clipboard equality guard against stale
or truncated results. Every accepted receipt also verifies in-memory restoration
of the original clipboard representations. A newer external copy is preserved.

The [full protocol and exact source diffs](../Benchmarks/Dictionary/PasteAudit/Host/)
disclose memory-only preferences, isolated home, startup controls, pre-timer
modifier normalization, source-event metadata and the old build’s extra owned-PID
paste guard. Physical input/focus changes abort capture. The legacy queue handle
uses a stopped-recorder private offset probed on macOS 26.6.2 / `25G83`, guarded
against other builds; this harness code is not part of the product. Apple’s
[Audio Queue Services](https://developer.apple.com/documentation/audiotoolbox/audio-queue-services)
provides the processing-tap API; the installed SDK describes input processing
before encoding and implicit tap disposal with the queue. The old tap may change
Audio Queue buffering (observed maximum block: 1024 frames); this is not a claim
of zero instrumentation cost or exact unmodified-release buffering. Its implicit
tap destruction happens inside real Stop and adds unmeasured old-only teardown;
small five-second differences cannot establish a faster shipping-app verdict.

The hardened private source apps have no audio-input entitlement. Their existing
permission attribution does not prove acoustic acquisition; the input can be
silent before replacement. Old uses default input, current built-in input; a
read-only query during the suite confirmed default UID `BuiltInMicrophoneDevice`.
Memory defaults override the shortcut to Control-Shift-K, but the host uses file
controls. Request UUIDs are driver bookkeeping, not independent correlation;
source PID, ordered capture generations, event-tagged clipboard generation and
actual payload provide the freshness evidence. Every final receipt also passes
reference-completed length, final-phrase and cross-build length checks.
Modifiers were normalized up, so the production 1.5 s wait branch is not tested.
Power/thermal state and the installed app's event-tap state were not recorded.
Fixed length order and three repetitions limit the distribution evidence.

Both source apps are private Developer ID builds with the existing app’s
designated requirement and Launch Services attribution; no host permission grant
or TCC edit was performed. `AuditDefaults` handles typed reads and writes in
memory because fixed-home variables alone did not isolate the preferences daemon.
The installed app is not used as a benchmark target. No host driver is installed.
Earlier route/acoustic dry runs failed payload validation and are excluded.

## Guest physical-loopback validation — VM only

**Eight of eight generation-guarded validations passed.** The
first 24 trials had actual `NSTextView` insertion receipts, but did not record
clipboard generation or require a spoken marker. They are now **provisional,
not acceptance performance results**. All 24 raw values, including the 93.317 s
outlier, are retained in `provisional-unguarded-paste.json`; do not use their
medians as the final comparison.

Fresh fixtures prepend a public synthetic spoken marker, retain exact 5/30/120/600 s
lengths and use identical SHA-256-verified WAVs for the two builds. Each capture
has a unique request UUID. The driver first puts an unrelated UUID sentinel on
the **guest** clipboard, then requires a later clipboard generation, exact equality
between clipboard and receiver text, the expected audio-derived marker in both,
and a target timestamp after the actual Stop event. No marker is appended by the
app or receiver. Apple documents [`changeCount`](https://developer.apple.com/documentation/appkit/nspasteboard/changecount)
as an ownership-generation counter and [`NSText.paste`](https://developer.apple.com/documentation/appkit/nstext/paste%28_%3A%29)
as consuming the general pasteboard. Counter advancement alone is insufficient;
the guard also verifies actual marker-bearing payload equality. App code remains unchanged. One fresh paired validation per
length completed. This establishes freshness, not a latency distribution.

| Marked fixture | v0.1.1 stop → insertion | Current stop → insertion | Fresh clipboard + receiver |
|---|---:|---:|---|
| 5 s | 1262.248 ms | 1444.411 ms | PASS |
| 30 s | 1878.834 ms | 1852.359 ms | PASS |
| 120 s | 5629.966 ms | 5717.876 ms | PASS |
| 600 s | 25240.133 ms | 49240.085 ms | PASS |

These are single observations in a four-vCPU guest, not native M5 latency or
stable old/new performance estimates. The current build was observed slower at
600 s in this VM; the experiment does not identify a cause or establish a native
regression. All eight request UUIDs are unique, each payload matches its clipboard,
and each old/current pair has the same input hash. The final clipboard ownership
transitions were 3→4, 5→6, 7→8, 9→10, 13→14, 15→16, 17→18 and 19→20.
Raw complete payloads, hashes, counters and timestamps: `guarded-paste.json`.

During the earlier unguarded series, current 600 s results varied from 32.549 to
93.317 s. A later guest snapshot recorded WindowServer at 67% CPU during capture;
this is load evidence, not proof of the outlier's cause. All unguarded numbers
remain explicitly excluded from acceptance conclusions.

The disposable Tart guest runs macOS 15.7.7, four virtual CPUs and 8 GiB on the
same M5 host. BlackHole routes the exact synthetic WAV through the actual microphone
capture code. The unchanged apps record, stop, transcribe, prepare and paste using
Command-V into a private `NSTextView`. Its receipt is recorded after `super.paste`
returns with inserted text. This is not a file-ASR substitute.

The guest uses Control-Shift-K toggle (rather than physical Option), manually
seeded test permissions, raw formatting, an empty dictionary and a copied model
cache. Startup/model loading are outside the warm timer. The timer begins before
posting stop-key-down and ends at completed text insertion, including real recorder
stop and delivery. It does not measure compositor rendering, human microphone
accuracy, onboarding, physical keys, T3/browser editors or rich-image paste.
The tested Parakeet capture paths do not auto-stop at 600 s: v0.1.1 uses
`AVAudioRecorder.record()` without a duration, and current `MicrophoneRecorder`
has no time cap. The 600 s storage bound in `NemotronStreamingAdapter` is not
used by this Parakeet test. The driver therefore posts the actual manual stop.
No app pipeline method is replaced. VM Core ML performance is not native M5
performance, so these values cannot validate a native 118.5 ms historical claim.

The installed host app identifies source `9707d70` (internal version 0.1.1);
it was neither replaced nor used as the benchmark target.
Current test source is `85dde51`; public v0.1.1 is identified by the released ZIP's
SHA-256 and executable UUID in the provenance receipt. Current is a private ad hoc
signed bundle; test grants have no code requirement and are not signature-upgrade
or onboarding evidence. The apps use the same pinned FluidAudio version.
The builds run in blocks, old first and current second, without randomization.
Playback WAV bytes match; capture timing and resampling can vary. Guest work ran separately from the native benchmark;
its guest was stopped before native timing or other audit work.

Setup trials with missing permissions, stale shared-folder executables or a target
without an Edit/Paste menu were discarded. Sources and the reproducible protocol
are in `Benchmarks/Dictionary/PasteAudit/`. The legacy target-v2 lacked the later
guest-account and generation guards; accepted trials use the compiled
`GuardedTarget` and `GuardedStopDriver` sources unchanged.

The filmed August 18 source commit is unknown. The fallback baseline is
public v0.1.1, exact source `c72f83415b1d760c30316f07fc776f09b0ebdc0b`.
The video's 118.5 ms Koett paste-post figure and 481 ms Wispr finished-state figure
use different endpoints and cannot be treated as target-insertion measurements.

## Regression attribution and cache follow-up

The original 24 receipts are **before the login-cache fix** and remain byte-for-byte
unchanged from `3998053`. They are not a final four-length matrix for PR #110.
[PR #110](https://github.com/oEdyb/koett/pull/110), based on #105, changes only
login-status refresh timing in production. It retains the VAD and recording,
transcription, correction and paste contracts.

### Same frozen WAV: component ablation

Sixty native warm trials used the same marked 5/30/120/600 s WAVs: three repeats
of legacy URL, its read/ASR split, current read/VAD/join/ASR split, production
without VAD, and normal production. Mode order alternated within each length.
All 60 outputs match per length, and split/production outputs match. Every VAD
trial retained every input sample on these dense fixtures. Values below are
milliseconds; component columns are measured in the same split invocation,
not added to manufacture a Stop-to-target result.

| Audio | Legacy URL total | Current bounded read | VAD | Join | Current ASR | Current production total |
|---|---:|---:|---:|---:|---:|---:|
| 5 s | 71.130 | 0.160 | 3.637 | 0.025 | 71.287 | 75.076 |
| 30 s | 165.205 | 0.364 | 21.223 | 0.159 | 181.006 | 201.626 |
| 120 s | 454.283 | 1.024 | 82.655 | 0.642 | 452.935 | 537.762 |
| 600 s | 2019.727 | 4.917 | 430.445 | 2.852 | 2046.233 | 2564.120 |

The forced full-file converter ablation at 600 s took 12.578 ms versus current
bounded read 4.917 ms;
source-timing remap was 0.031 ms and plain preparation 2.670 ms. At 120 s the
current read/VAD/ASR medians were 1.024/82.655/452.935 ms. The current no-VAD
production totals were 68.749/162.912/452.955/2126.074 ms. Actual recorded current
WAV headers are 16 kHz mono Float32 (format code 3), despite the 48 kHz input
callback. The frozen source fixtures are 16 kHz PCM16, converted to Float32 on
read. No sample-rate ratio change is required. The forced full-file converter's
equal-rate resampler returns its input. Actual legacy URL transcription defaults
to the disk-backed `AudioSourceFactory` route above 480,000 samples (30 s), which
uses an AVAudioConverter and a temporary mapped Float32 file. Its preparation
was not separately timed inside the actual URL invocation; the URL totals above
are direct measurements, while the full-file read timer is an explicit ablation.
Both routes use the same model and ChunkProcessor; they do not have identical
file preparation. A 48 kHz post-Stop resampling hypothesis is rejected.
See `attribution/stage-profile*.json`, the opt-in `DictationStageProfileTests`,
and pinned `AudioConverter.resampleAudioFile`/`AsrManager.transcribe` source.

The additional dense-audio work is predominantly the **full Silero VAD scan**:
82.655 ms at 120 s and 430.445 ms at 600 s, where it removed no samples. It is a
deliberate silence-handling cost, not dictionary boosting or quadratic sketch
alignment. `SketchLinking.keyframeSummary` returns immediately with no anchors;
plain preparation remains small. The main model and pin are identical.

Removing VAD is not a safe equivalent fix. The existing native sparse-speech
regression test kept all three sentences and reduced 364 s to 7.9 s of retained
audio: 1971 ms without trimming versus 370 ms with it. In that synthetic case,
“Call the dentist” also replaced the untrimmed “Cool, the dentist.” All-silence
and natural-pause tests pass. Moving equivalent VAD work into capture would need
ordered resampling/state/cancellation validation; it is outside this small patch.

### Actual Stop: identified blocking lookup and rejected teardown idea

Private production-stage probes measured about 41–42 ms rebuilding the menu;
about 40 ms was `SMAppService.mainApp.status`, while microphone-menu work was
under 1 ms. Input Stop plus file close took about 4–6 ms; stop-sound playback
setup about 18–21 ms. Audio-unit destruction took about 0.4 ms. Retaining the
already-stopped recorder until after delivery moved its destruction but did not
explain or fix the delay. That prototype was rejected after seven valid receipts;
its next planned recording was canceled before starting, with clipboard cleanup.

PR #110 caches login status for automatic busy-state menu updates. Startup's
first status, ready transitions, explicit setting actions and explicit menu
opening still refresh. A menu opened while busy can therefore still synchronously
read status for about 25–40 ms; this is **not** a promise never to query while busy.
Two ready-state reads after paste remain. The helper tests use read counts and
external-status changes. Their name now says explicit refresh; controller
menu-opening wiring received static Opus review and is not event-dispatch tested.

### Separate historical before/after cohorts and systematic work shift

| Cohort | 5 s Stop → target values | 5 s median / worst | 120 s observation |
|---|---|---:|---:|
| Cache ablation, baseline | 155.827 / 152.270 / 162.776 ms | 155.827 / 162.776 ms | 615.980 ms |
| Cache ablation, cached | 145.279 / 137.546 / 146.051 ms | 145.279 / 146.051 ms | 592.561 ms |
| Final production cache implementation | 165.586 / 137.506 / 181.339 ms | **165.586 / 181.339 ms** | **638.169 ms** |

All three short measured baseline ASR/frontend stages were 77.1/78.6/84.0 ms,
versus 108.3/100.7/103.6 ms for cached. Mean Stop-to-ASR-start improved 37.3 ms,
but mean ASR/frontend **rose 24.3 ms**; mean target gain was about 14.0 ms in that
ablation. Including warm-ups, all seven baseline 5 s ASR stages were 77.1–84.0 ms,
while all fourteen cached/final-production stages were at least 100.2 ms.
This systematic **contention or work shift remains unexplained**. It must not be
called merely random noise. The production code leaves the ASR path intact,
but the earlier start changes overlap/scheduling conditions that were not isolated.

The final production cohort's 5 s median is 166 ms, not the original before-cache
matrix's 158 ms. Its single 120 s result is not a repeated distribution. These
results establish removal of the blocking lookup, **not a guaranteed end-to-end
gain, a 10% ceiling, or that the full v0.1.1 regression has been fixed**.

### Exact harness, provenance and separate hash-bound recheck

[Attribution sources and reproduction protocol](../Benchmarks/Dictionary/PasteAudit/Attribution/)
include the exact executed collectors, driver with capture-ready hook, resource
helper, shared runtime, and full cache/production source diffs on top of the
original controlled-input patch. Both patches reconstruct the rebuilt cache and frozen production Swift
source trees byte for byte. The portable build-only preparer
is supplied; its combined clean command was not run end-to-end. Individual
build/sign/launch steps and source reconstruction were executed.

The historical prototype binary was overwritten before hashing; **no historical
cache/baseline binary attestation is claimed**. Its source was reconstructed from
the recorded transformations. The final production executable was frozen and
hashed after its historical cohort, then reused in the new pre-hashed recheck.
Historical limits are explicit in `attribution/historical-provenance.json`.

A separate nine-receipt native recheck binds source manifests, app binaries,
driver source/binary, collector, resource helper, shared runtime and fixture
object/source hashes before launch. Source and binary files were checked again
afterward. It reuses the frozen old and production executables, plus a rebuilt
cache-ablation executable. It does not retroactively hash the overwritten binary.
All nine passed; all payloads match per length and clipboard restoration passed.

| New hash-bound cohort | Observations | Median / worst |
|---|---|---:|
| v0.1.1 fallback, 5 s | 156.567 / 147.359 / 153.500 ms | 153.500 / 156.567 ms |
| Same final production binary, 5 s | 146.304 / 135.988 / 184.241 ms | 146.304 / 184.241 ms |
| Same final production binary, 120 s | 636.110 ms | Single observation |
| Rebuilt ablation baseline / cached, 5 s | 154.582 / 152.267 ms | Single pair |

These are a separate integrity/resource cohort, not a replacement or pooled
extension of the earlier performance distributions. No final 30/600 s cache-fix
capture matrix was run. Complete bindings, counters and source manifests are in
`attribution/attested/`; all 28 continuation receipts (7 rejected-design, 8 cache
ablation, 4 final production, 9 integrity/resource) pass the committed verifier.

### Native resources: definitions and every fresh process

All counters describe the owned private empty-dictionary process under the PCM
seam, including fixture filling and private control polling. CPU is process
user+system time divided by wall time, expressed as a percentage of one core;
it excludes Core Audio/WindowServer processes and accelerator utilization.
RSS and physical footprint exclude unaccounted shared/accelerator memory.
They are not a Mac long-session soak or total model-memory claim.

Idle counters follow one complete 5 s warm-up dictation and three idle seconds.
Recording counters sample only while fixture injection is incomplete and the
source reports recording, **before measured Stop**. The earlier four production
resource cases have no post-measured-ASR counter. The new recheck waits for the
source to return ready after target insertion, then samples three idle seconds;
its post-target counters therefore include completion of that measured ASR and
successful delivery. No earlier recording sample is labeled post-Stop.

| Fresh cohort | Launch → Ready log, ms | First injected buffer observed, ms | Idle CPU, % of one core | Recording CPU, % of one core |
|---|---|---|---|---|
| Old 3 × 5 s | 1069.557 / 578.234 / 484.738 | 179.721 / 184.337 / 202.283 | 0.044 / 0.042 / 0.047 | 0.274 / 0.290 / 0.256 |
| Current 3 × 5 s + 120 s | 1005.029 / 660.633 / 543.369 / 555.906 | 101.007 / 92.879 / 97.725 / 104.941 | 0.054 / 0.056 / 0.056 / 0.054 | 0.319 / 0.339 / 0.308 / 0.359 |

Old idle RSS MiB: 118.891 / 121.359 / 121.344; maximum sampled recording RSS: 119.422 / 121.891 / 121.906; settled post-target RSS: 120.625 / 120.578 / 120.625; settled physical footprint: 34.188 / 34.548 / 34.423.

Current idle RSS MiB: 156.125 / 156.297 / 156.312 / 156.219; maximum sampled recording RSS: 172.391 / 172.578 / 172.625 / 181.219; settled post-target RSS: 155.562 / 156.594 / 156.625 / 197.906; settled physical footprint: 41.282 / 41.173 / 41.391 / 58.001.

The old and current readiness hooks both observe the first nonzero injected
fixture-frame counter via private progress polling after controller Start. The
old callback is an Audio Queue processing tap; current is the AUHAL input callback.
These are **control request to first injected-buffer observation**, not physical
microphone acquisition or equal hardware buffering. Launch-to-Ready includes
cached-model setup/warm-up and existing permissions; old per-model load was not
separately instrumented. Current model-load times were 129.534/182.197/141.407/
131.394 ms, with ASR warm-up 100.176/87.161/77.516/77.400 ms in matching order;
console logs retain the original values. Caches were not cleared and all individual launches are shown. An earlier
exploratory launch was 13.142 s; its complete resource file was overwritten during
probe reuse. That retained observation is separate from these cached-launch
cohorts and does not establish cold-start performance.

Logical private app sizes were 18,281,342 bytes old and 21,145,034 bytes current.
Selected v2 model files total 464,413,250 bytes; Silero VAD 1,063,427 bytes.
The 102,803,869-byte CTC cache, v3 and Nemotron caches remain on disk but were not
loaded for these empty-dictionary runs. Logical bytes do not measure incremental
APFS clone space. Full counters and per-process conditions remain in raw JSON.

### Performance release recommendation

Keep VAD and its measured silence-handling benefit. The dense-audio scan cost
remains about 83/430 ms at 120/600 s and cannot be safely removed while preserving
that contract. Keep the small reviewed login-cache change to avoid synchronous
service queries on automatic busy updates, with fresh explicit-menu/ready reads.
Keep #105's authorized dictionary speed/recall tradeoff.

Do **not** describe this candidate as universally faster than v0.1.1, advertise
118.5 ms target insertion, or call the entire regression fixed. Cache removal of
the blocking lookup is proven; its ASR work shift and end-to-end variation remain.
The original 24 before-cache receipts and all later cohorts stay separate.
This performance work supports those specific code changes; broader clean-install,
permission, drawing and target capability release gates belong to the separate
release audit. No release was published or installed by this audit.

Continuation validation: 438 tests, 13 optional skips, zero failures (43.762 s);
release build passed (57.99 s), required diff/shell/plist checks passed. Renamed
spy tests passed 3/3. Opus 5.5 approved production code at `d1b689a` with no runtime
or cache-contract defect; its disclosure, harness and test-name findings are
addressed here. Later runtime code is unchanged. Original dictionary/#105 gate
receipts below retain their original source/count scope.

## Verification

- `swift test`: 434 tests, 12 optional tests skipped, zero failures (44.565 s).
  Covers default-off behavior and the existing contracts; not a target paste receipt.
- `swift build -c release`: passed (58.71 s); no host installation performed.
- `git diff --check`, `zsh -n install-macos.sh`, both packaging plist lints: passed.
- A read-only Opus 5.5 review of `5ef3c30` found no runtime/default correctness
  blockers; its receipt, test and disclosure findings are addressed in this report
  and the final verifier. The subsequent changes have not had another model review.
  No CI watch or host installation was performed.

## Decisions and limits

The normal app keeps cheap spelling, explicit sounds-like forms and exact replacements.
The separate acoustic pass is available only to explicit experimental callers;
there is no new app toggle. Dependency pins and the selected/default model are
unchanged. The host installation and its data were not changed.

Native host measurements use the disclosed recorder-input seam and actual
Stop/close/delivery/target receipt. They do not establish microphone acquisition
or physical-key latency for an unmodified release binary. The guest validations
retain the actual capture route but have VM Core ML performance. Neither series
is constructed by adding separately measured file-ASR and paste durations.

Private reproducible outputs live in the audit worktree's `.build/performance/`:
`guarded-runs.log`, `guarded-paste.json`, `guarded-fixtures/`, `full-paste.log`,
`full-paste.json` (provisional), `corpus/`, native timing JSON and gate logs.
The native controlled-input suite writes its raw receipts, warm-ups, source logs
and provenance beneath `.build/performance/host/`. No private dictionary or History
export was used. The exclusive native measurement slot
was held through the native suite and final verification, then explicitly released. The source apps and
receiver exited after verified clipboard restoration; the guest remains stopped.
