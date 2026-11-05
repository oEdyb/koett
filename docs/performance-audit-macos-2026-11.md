# Mac performance audit — November 2026

Status: native dictionary and 210-clip corpus measurements complete; actual-target
paste comparison in progress. No installed host app, preferences, history, clipboard
or focus changed. All transcripts in the committed receipts are public synthetic fixtures.

## Scope and source

M5 MacBook Air, macOS 26.6.2 (Darwin 25.6.0), native arm64 release tests. Source
`c0343ae` before the default-disable change. FluidAudio remains exactly 0.15.6,
revision `4dbf4f9f9a5ff3a53ade848d7ba4e3df13db859b`. No dependency upgrade.

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

The release worker stopped its VM and held builds/model work for this slot.
The benchmark used an exclusive `/tmp/koett-mac-benchmark.lock` file lock. The guest
was suspended during native timing. Ordinary desktop processes remained active;
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

## Decision

Disable acoustic boosting by default; keep spelling, sounds-like forms and exact
replacements. Only explicit experimental/benchmark callers may enable CTC.
The tested cheap path meets the 10% latency budget at every length. It does not
preserve CTC's term recall at 30 and 120 s; this is a speed/recall
tradeoff. Both original requirements were not achieved.

## Old/current actual-target paste

**Acceptance validation is being rerun with explicit generation guards.** The
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
app or receiver. App code remains unchanged. One fresh paired validation per
length is running; this establishes freshness, not a latency distribution.

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
Playback WAV bytes match; capture timing and resampling can vary. The series
pauses at a saved receipt boundary for the release auditor's gates;
its guest is stopped before native timing or the auditor's work.

Setup trials with missing permissions, stale shared-folder executables or a target
without an Edit/Paste menu were discarded. Sources and the reproducible protocol
are in `Benchmarks/Dictionary/PasteAudit/`. The target source adds a guest-account
guard after the measured target-v2 build; insertion/receipt behavior is identical.

The filmed August 18 source commit is unknown. The fallback baseline is
public v0.1.1, exact source `c72f83415b1d760c30316f07fc776f09b0ebdc0b`.
The video's 118.5 ms Koett paste-post figure and 481 ms Wispr finished-state figure
use different endpoints and cannot be treated as target-insertion measurements.

## Verification

- `swift test`: 432 tests, 11 optional tests skipped, zero failures (45.057 s, final isolated-home run).
  Covers default-off behavior and the existing contracts; not a target paste receipt.
- `swift build -c release`: passed (57.07 s, final isolated-home run); no host installation performed.
- `git diff --check`, `zsh -n install-macos.sh`, both packaging plist lints: passed.
- Independent review is pending. The PR remains a draft while measurements finish.
