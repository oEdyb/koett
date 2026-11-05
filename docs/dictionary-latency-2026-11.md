# Dictionary latency and recall — November 2026

## Acceptance and release policy

The acceptance target is about **10% or less added latency at 5 s, 30 s,
2 min and 10 min**, with the same or better recovered-term recall on identical
benchmark clips. Ordinary words must survive; the feature contract forbids
shipping an unrestricted single biased stream. If no candidate qualifies, normal
dictation uses one greedy acoustic pass and keeps cheap spelling, explicit
sounds-like forms, exact-term correction and literal replacements. The old slow
acoustic path remains available to explicitly selected benchmark callers.

No dependency version or default speech model has been upgraded for this work.
Nothing here is a claim that a production binary has been installed or released.

## Research checked 2026-11-05

The installed pins are sherpa-onnx **1.13.5** (native source
`3dc7c569f31ca2cd4a20ed6f7db780327e6714c5`) and FluidAudio **0.15.6**.
GitHub's release API reported sherpa-onnx **2.0.1**, published October 26, and
FluidAudio **0.18.2**, published November 1. Latest documentation is research,
not evidence that the pinned dependency implements every new API.
[sherpa release](https://github.com/k2-fsa/sherpa-onnx/releases/tag/v2.0.1),
[FluidAudio release](https://github.com/FluidInference/FluidAudio/releases/tag/v0.18.2).

- **Single-pass transducer hotwords already exist.** Sherpa integrates an
  Aho–Corasick context graph into modified beam search. Koett added a separate
  plain stream to protect ordinary words. Its batch call contains two copies
  of the same audio: one native encoder invocation processes a batch of two,
  rather than reusing one encoded representation. Both the pinned and 2.0.1
  NeMo implementations construct encoder features per stream. Merely upgrading
  the library does not introduce cross-stream encoder sharing.
  [Hotwords documentation](https://k2-fsa.github.io/sherpa/onnx/hotwords/index.html),
  [pinned NeMo source](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.5/sherpa-onnx/csrc/offline-recognizer-transducer-nemo-impl.h),
  [2.0.1 source](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/sherpa-onnx/csrc/offline-recognizer-transducer-nemo-impl.h).
- **CTC word spotting is the relevant encoder-reuse method.** The NVIDIA paper
  matches CTC frame probabilities against a context graph and replaces matching
  frame intervals. A transducer needs a compatible hybrid CTC head to use that
  representation. CPU timing on Koett is not established by the paper's results.
  [Fast Context-Biasing with CTC-based Word Spotter](https://arxiv.org/abs/2406.07096).
- **FluidAudio distinguishes shared and separate encoders.** Current
  `VocabularyBoostingSession` documentation describes a shared CTC projection
  for the hybrid 110M model, but a separate CTC encoder for pure 0.6B v2/v3.
  Its text preflight avoids CTC inference only when no candidate needs evidence;
  small-vocabulary acoustic rescue can conservatively require inference even
  without a text candidate. Disabling rescue changes recognition behavior.
  These are vendor architecture descriptions, not measured Koett overhead.
  [FluidAudio vocabulary documentation](https://github.com/FluidInference/FluidAudio/blob/main/Documentation/ASR/CustomVocabulary.md).
- **Post-correction has a cheap shipping precedent.** Handy calls
  `apply_custom_words` on the recognized text. This establishes a text-stage
  approach, not equivalent acoustic recall or measured Koett latency. Koett's
  existing spelling/forms pass is the conservative option; adding broad phonetic
  guesses risks changing common words such as “cloud” into “Claude”.
  [Handy source](https://github.com/cjpais/Handy/blob/main/src-tauri/src/managers/transcription.rs).

Running two encoders concurrently can hide some wall time with spare CPU, but
retains duplicate work, memory and contention. A text-only candidate gate can
miss terms absent from greedy output. Neither is assumed to preserve recall.
CTC word spotting on the existing 110M logits would require an additional native
API/decoder integration; Koett's current sidecar instead runs the larger v2
model and is not an encoder-reuse solution.

## Reproduced cause and term-count scaling

Original engine, copied v2 model, identical 5-second synthetic WAV, one warm-up
followed by five measured repetitions, two CPU threads. List sizes beyond the
20-term starter list use synthetic distractor terms; this tests cost, not recall.

| Terms | Warm median ASR ms | Worst ASR ms |
|---:|---:|---:|
| 0 | 148.1 | 153.0 |
| 1 | 333.3 | 344.7 |
| 20 | 300.1 | 360.7 |
| 200 | 306.6 | 358.4 |
| 2,000 | 309.5 | 311.8 |

Load was approximately 2.65–2.86 during these sequential conditions. This does
not isolate a tiny trie-size effect. The major discontinuity is **zero to one
term**, when Koett switches from one greedy stream to two beam-search streams.
The original 5/30/120/600-second measurements and their limitations remain in
the [workflow audit](https://github.com/oEdyb/koett/pull/102).

### Shared-encoder term-count repeat

Same 5-second WAV, one warm-up and five measured runs per list, two threads,
sequential list sizes; load was 0.98–1.30. Entries beyond the starter list are
synthetic distractors. This repeat measures latency, not large-vocabulary recall.

| Terms | Shared-build median ms | Worst ms |
|---:|---:|---:|
| 0 | 142.3 | 143.7 |
| 1 | 156.1 | 159.8 |
| 20 | 156.4 | 157.9 |
| 200 | 157.6 | 160.2 |
| 2,000 | 174.4 | 189.5 |

The 20-term list adds 9.91% here; 2,000 terms add **22.56%**. Large vocabularies
therefore do not meet the nominal 10% target on this probe. The old and new
scaling runs used different host loads and native builds; do not treat their
row-to-row difference as a controlled speed ratio. The controlled three-mode
comparison below supplies that evidence for the starter list.

## Same-clip method comparison

Corpus: existing `Benchmarks/Dictionary/corpus.json`, 30 terms, 40 positive and
30 negative sentences in three synthetic voices: **210 WAVs, 162 expected term
occurrences**. Every clip must be present. Two CPU threads, pinned int8 v2,
copied model, network disabled, `nice -n 10`, 12 GiB scope. Method order rotates
per clip to spread host-load drift. One inference per method per clip; aggregate
timings below are not repeated single-clip latency distributions. The method
search uses this corpus, so it is a validation bake-off, not an untouched holdout.

Initial run with the original native libraries:

| Method | Correct terms / 162 | False terms on negatives | Worse negative clips | Lost non-term words | Total inference ms |
|---|---:|---:|---:|---:|---:|
| Raw greedy | 50 | 0 | 0 | 0 | 20,099.25 |
| Greedy + cheap spelling | 62 | 0 | 0 | 0 | 20,102.39 |
| Existing two-stream beam, 4 paths + adoption | 114 | 0 | 0 | 1 | 38,517.68 |
| Single biased stream, 4 paths + spelling | 118 | 0 | 4 | 11 | 21,123.87 |
| Single biased stream, 1 path + spelling | 78 | 0 | 4 | 7 | 20,098.19 |

Cheap spelling added **0.016%** to aggregate measured time. It recovered 12 more
term occurrences than raw greedy but **52 fewer than the old acoustic path**.
Single-stream 4-path biasing added **5.10%** and recovered more terms, but fails
the ordinary-word preservation contract. It is rejected. No broad phonetic
correction was added to conceal the lost recall.

## Encoder-reuse prototypes

A private native prototype first encodes identical stream features once, copies
the encoder tensor into the existing two-stream decoder and keeps adoption.
The upstream dependency and ONNX Runtime archive are unchanged. This native
build also recompiles preprocessing code with the current compiler; its raw
text differs on one corpus term from the older cached native build. Cross-build
text differences therefore cannot all be attributed to encoder sharing.

Within a repeat run of that prototype, raw/cheap/adoption results were 49/61/115
correct terms. Sharing the encoder with four-path beam decoding added **12.23%**
aggregate time relative to its own greedy control; two paths added **5.00%** but
recovered only **93/162**, losing recall. Neither establishes acceptance at all
four audio lengths.

The accepted explicit `koett_context` mode shares the encoder between a greedy
base decoder and one four-path biased beam decoder. Unlike a single-stream
replacement, the merge keeps plain words and takes only supported dictionary
terms. Upstream decoding modes remain unchanged. Normal transducer dictation
uses this mode when its linked native archives pass the capability check; CTC
and older native builds fall back to spelling only.

## Final native build and same-clip rerun

Patched NeMo header SHA-256:
`673caa2c3c3053158cc5c2fec576b17607ea0a053d648f34422591e0c689d920`.
The final 210-clip rerun records that fingerprint in its metadata. Results:

| Method, same final native build | Correct terms / 162 | False terms | Worse negatives vs raw | Lost ordinary words vs raw | Total ms |
|---|---:|---:|---:|---:|---:|
| Greedy | 49 | 0 | 0 | 0 | 19,741.13 |
| Greedy + spelling | 61 | 0 | 0 | 0 | 19,744.43 |
| Old two-stream acoustic path | 115 | 0 | 0 | 0 | 37,703.02 |
| Shared encoder + greedy base + biased decoder + adoption | 115 | 0 | 0 | 0 | 21,144.15 |

The accepted mode adds **7.09% aggregate inference time** over greedy plus
spelling, versus **90.96%** for the old path. It recovers the same term occurrences
as the old path. The plain stream matched greedy text exactly on all 210 clips.
A separate native probe with different input features matched the legacy
non-shared batch, verifying the guard's fallback.

One inference per clip gives overhead p50 **7.17%**, p90 **10.01%**, p99 **14.30%**,
maximum **24.18%**. Median added time is **6.59 ms**, maximum **23.21 ms**.
Percentiles use linear interpolation over the 210 per-clip percentage ratios.
This meets the aggregate target; it does **not** establish a 10% worst-case bound.
Context negative WER is 0.0028 versus the old path's 0.0016: the base is now
ordinary greedy, so this is unchanged from raw, not better than the old beam base.
These synthetic voices and this 30-term corpus were used during development;
there is no unseen human-speech holdout in these results.

## Production-path four-length acceptance

`dictionary_default_bench` uses `ParakeetTranscriber`, the same 20-term starter
list as the workflow audit, copied v2 model, two CPU threads, identical WAVs,
one warm pair and three alternating measured pairs per length. Its timer includes
`dictionary::apply`. It refuses to run without the verified shared capability,
and records mode, source fingerprint and output. It does not measure paste.

| Audio | Empty median / worst ms | Shared dictionary median / worst ms | Added median |
|---|---:|---:|---:|
| 5 s | 147.04 / 149.86 | 160.77 / 161.09 | 9.34% |
| 30 s | 816.14 / 816.47 | 864.13 / 885.20 | 5.88% |
| 2 min | 3,292.14 / 3,297.08 | 3,597.35 / 3,606.09 | 9.27% |
| 10 min | 16,373.00 / 16,417.18 | 17,814.69 / 18,028.00 | 8.81% |

**Decision:** enable shared-encoder biasing on verified transducer builds.
All four measured median overheads are below 10%, with unchanged acoustic term
recall on the same corpus. Keep the expensive old path benchmark-only. Native
libraries without the capability log `spelling_only` and cannot silently turn
on the expensive path. A stale capability marker fails the build before model
creation rather than calling an unsupported native decoder and exiting.

### Additional comparison under CPU contention

A later instrumented repeat rotates **three production modes** in one process:
greedy, accepted shared encoder, and the explicit old acoustic path. One warm
round and three measured rounds, identical copied model and WAVs, two threads;
per-inference load averages are retained in `production-three-modes.json`.
The private live recording was active and unrelated host tests also ran.

| Audio | Host 1-minute load range | Greedy median ms | Shared median ms | Old acoustic median ms | Shared added vs greedy | Less time vs old |
|---|---:|---:|---:|---:|---:|---:|
| 5 s | 16.86–17.99 | 231.67 | 254.27 | 490.95 | 9.76% | 48.21% |
| 30 s | 17.76–19.13 | 891.54 | 1,026.92 | 1,707.19 | 15.18% | 39.85% |
| 2 min | 9.77–16.50 | 3,803.51 | 4,094.31 | 6,866.58 | 7.65% | 40.37% |
| 10 min | 5.02–10.18 | 19,063.37 | 19,576.54 | 35,042.28 | 2.69% | 44.13% |

**Counter-evidence:** the 30-second contended result exceeds 10%. The initial
four-length acceptance did not log load per pair; the adjacent corpus run logged
2.76 / 2.36 / 3.56, which is not proof of the production run's load. Do not claim
a universal latency bound, or treat the contended repeat as passing all lengths.
The default decision uses the initial median acceptance plus preserved term
recall and the removal of duplicate encoder work. It is qualified by these
contended results and short-clip tails. It cuts 39.9–48.2% from the old acoustic
path's measured median time in this loaded repeat; it does not promise that
custom vocabulary is free. CTC, which cannot reuse this encoder representation,
uses the cheap fallback instead.

**110M tradeoff:** normal CTC uses spelling only; it no longer starts the separate
v2 sidecar. A fresh same-corpus rerun measured raw **27/162**, spelling **45/162**,
and the explicitly selected bounded sidecar plus spelling **111/162**. Disabling
the sidecar therefore loses **66 recovered occurrences** on these synthetic
clips. Both outputs introduced zero false terms, worsened negative clips or lost
ordinary words against raw. This is a substantial recall cost, not equivalent
custom vocabulary support.

The CTC comparison used the copied 110M and v2 models, 4-thread raw/bake-off
engines, then the production 2-thread CTC / 4-thread sidecar deadline harness,
network disabled and the same 210 WAVs. It ran while the private live recording
was active; do not compare its timings with the quiet transducer acceptance.
Within the same deadline harness, an 18.59 s probe took **183.63 ms raw** versus
**415.56 ms bounded** (+126.3%), returned unchanged raw text after its deadline,
and the immediately overlapping request returned in **80.80 ms** while the
sidecar remained busy. This fails the 10% target, so it stays benchmark-only.
Current real-model regression tests verify no sidecar starts in the default
mode even when a copied v2 model exists. The
[historical CTC benchmark](ctc-dictionary-benchmark.md) remains separately dated.

## Receipts and checks

Raw public-corpus outputs, production measurements and native fingerprint are in
[Benchmarks/Dictionary/2026-11](../Benchmarks/Dictionary/2026-11).
No private transcript store was exported. The final native core was compiled
before the final corpus and four-length measurements.

Executed: 426 Rust library tests passed (21 optional tests ignored), then all
three real-model hotword tests passed separately using copied v2 and CTC models;
format and Clippy passed. The native patch tests verify exact pinned input,
idempotence and refusal of unknown source. The stale-archive build probe rejected
a deliberately changed private archive before linking. Linux release build
passed. Local Windows checks cannot complete: native Windows archives and MSVC
`lib.exe` are absent; Windows CI remains required. No Windows runtime result or
Mac result is implied by these Linux checks.

The end-to-end live paste, feature and memory results belong in the
[workflow audit](https://github.com/oEdyb/koett/pull/102); ASR timings above must not be used
as release-to-visible-text claims.
