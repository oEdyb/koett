# Mac performance audit — November 2026

Status: native dictionary measurements complete; corpus recheck and actual-target
paste comparison in progress. No installed host app, preferences, history, clipboard
or focus changed. All transcripts in the committed receipts are public synthetic fixtures.

## Scope and source

M5 MacBook Air, macOS 26.6 (Darwin 25.6.0), native arm64 release tests. Source
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

## Decision

Disable acoustic boosting by default; keep spelling, sounds-like forms and exact
replacements. Only explicit experimental/benchmark callers may enable CTC.
The tested cheap path meets the 10% latency budget at every length. It does not
preserve CTC's term recall at 30 and 120 s; this is the user's explicitly authorized
speed/recall fallback, not a claim that both original requirements were achieved.

## Old/current actual-target paste

Pending. The filmed August 18 source commit is unknown. The fallback baseline is
public v0.1.1, exact source `c72f83415b1d760c30316f07fc776f09b0ebdc0b`.
The video's 118.5 ms Koett paste-post figure and 481 ms Wispr finished-state figure
use different endpoints and cannot be treated as target-insertion measurements.
