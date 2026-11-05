# Mac performance audit — November 2026

Status: native dictionary latency, corpus recall and vocabulary-switch memory checks
are verified. The repeated native M5 controlled-input Stop-to-target suite is in
progress. Eight guarded guest validations also passed. The installed host app,
preferences and History were not changed. Host target focus and clipboard were
used in a private probe; clipboard backups stay in memory and are restored after
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

All length medians remain below the 10% budget. One individual 120 s pair was
+24.80%; the other two were −6.62% and +1.44%. This does not establish a
per-recording 10% ceiling. Negative differences are run variation,
not evidence that spelling speeds up ASR. All 12 default transcripts exactly
matched the first sweep's spelling-only transcripts. Raw repeats and equality
checks are in `default-latency.json` and `default-summary.json`.

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

| Constructor | Default RSS range | Loaded CTC RSS, first → fourth | Empty RSS after load, first → fourth |
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

## Native M5 controlled-input Stop → target

The host comparison uses source builds of public v0.1.1 `c72f834` and current
`85dde51`, both with the same pinned FluidAudio, raw formatting and empty
dictionaries. The model and a complete five-second pipeline are warmed before
each measured group. Three groups per build alternate old/current, current/old,
old/current. The 24-capture suite is still running; no final median is claimed yet.

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
before encoding and implicit tap disposal with the queue.

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

## Verification

- `swift test`: 433 tests, 12 optional tests skipped, zero failures (62.375 s).
  Covers default-off behavior and the existing contracts; not a target paste receipt.
- `swift build -c release`: passed (59.29 s); no host installation performed.
- `git diff --check`, `zsh -n install-macos.sh`, both packaging plist lints: passed.
- Independent review is pending. No CI watch or host installation was performed.

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
export was used. The guest is stopped; the exclusive native measurement slot
remains held until the native suite and cleanup finish.
