# Parakeet 110M acoustic dictionary bias

Measured 2026-11-04, Linux x86_64, Ryzen 5 9600X, CPU execution. Rust 1.94.0;
pinned sherpa-onnx 1.13.5 and existing int8 model manifests. No dependency upgrade.
No new audio was synthesized or sent to a service for these measurements.

## Decision and behavior

Use an optional **Parakeet v2 transducer sidecar**, not a claim of native CTC
hotwords. The primary 110M decoder stays greedy. With usable dictionary terms
and an already-downloaded, verified v2 model, one background worker loads v2
once. Its real hotword decode (score 2, four beam paths, plain + biased streams)
uses the same audio. Conservative term adoption keeps the CTC transcript except
for dictionary terms supported by that candidate. Spelling replacement is a
separate stage and is measured as a separate control. Terms containing the
upstream hotword grammar separators `/` or `:` keep spelling support but are
excluded from acoustic hotwords, as in the existing transducer path.

The sidecar never downloads automatically. To make it available, download v2
through the existing model picker and switch back to 110M. Dictionary terms then
activate it; a missing or invalid model leaves CTC working. `menu-status` reports
`ctc_dictionary_bias`. Advanced local runs can point `KOETT_CTC_BIAS_MODEL` at a
copied v2 folder; its files must still match the pinned v2 manifest.

The sidecar has a **budget per decode unit of 250 ms plus 40 ms per second of audio,
capped at 1 s**, including dictionary update,
decoding and adoption. Loading, busy, failed, oversized, disconnected, expired or
late results retain the exact primary CTC text at the ASR boundary. That raw text
still receives the independent user spelling/formatting/replacement policy.
This is not a 250 ms limit on the primary ASR or on a whole multi-segment file.
A unit that misses its budget deliberately falls back.

Native ONNX decoding cannot be interrupted safely through this API. There is
one cached worker per process, at most one active request and a bounded channel;
a stalled request prevents further submissions rather than accumulating work.
Late results go only to their dropped request receiver, never another session.
An in-flight job holds at most one million samples in memory, no audio file.
The extra model stays cached until application exit, even after clearing the
words. Clearing the dictionary stops sidecar requests; it does not reload the
primary model. A failed worker stays disabled until restart. No cloud or recovery
retranscription is used for a sidecar failure.

## Corpus and methods

Existing `Benchmarks/Dictionary/corpus.json`: 30 terms; 40 positive sentences and
30 negative/near-miss sentences, each rendered in three synthetic voices: 210
real WAV inferences. All expected clips must be present. Report contains corpus
and individual clip SHA-256 values, raw transcripts and timing observations.
Audio stays outside git.

Development: Andrew, 70 clips. Voice holdout: Emma + Ryan, 140 clips (80 positive,
60 negative), 108 expected written terms. The holdout shares sentences with the
development voice; **not unseen-text, human-microphone or accent acceptance**.
No aliases were added to improve these scores. Existing beam score/adoption
rules were retained. The development sidecar maximum was 167 ms; 250 ms leaves
headroom while limiting the optional latency cost. No per-clip tuning.

Exact case-sensitive written-term recall on the voice holdout:

| Method | Terms correct | Recall | New false terms | Negative transcripts changed |
|---|---:|---:|---:|---:|
| Raw 110M CTC | 17 / 108 | 15.74% | 0 | 0 |
| CTC + spelling only | 30 / 108 | 27.78% | 0 | 0 |
| CTC + unbiased v2 candidate adoption | 38 / 108 | 35.19% | 0 | 0 |
| CTC + biased v2 candidate adoption | 70 / 108 | 64.81% | 0 | 0 |
| Actual bounded sidecar + spelling | 76 / 108 | 70.37% | 0 | 0 |

The unbiased control uses greedy v2. The biased mechanism uses the existing
plain + hotword modified-beam decoder; the comparison does not isolate beam
search from hotword boosting. The bounded acoustic result itself matched the
unbounded acoustic result. Positive WER changed from 19.15% raw to 7.82% bounded;
negative WER stayed 2.05%. No measured collateral words or negative regressions.
Exact written-term recall includes spelling/case fidelity, not only phonetics.
Conservative common-word guards still reject some names (for example Cat/Koett).

## Timing and fallback checks

The bake-off used four threads per engine. The shipping-path benchmark used a
two-thread CTC primary and four-thread sidecar, with the real 250 ms wait:

- Unbounded biased sidecar median 135.8 ms, p95 159.9 ms, maximum 181.0 ms.
- Actual bounded pipeline median 157.5 ms, p95 187.6 ms, maximum 210.0 ms.
- Cold primary call returned in 61.6 ms while the sidecar loaded asynchronously.
- An 18.59-second repeated human-narrator model fixture: primary 159.6 ms,
  bounded call 400.1 ms, **exactly identical raw output**. The next overlapping
  request returned in 62.3 ms while the sidecar was still busy. This demonstrates
  fallback/backpressure; it is not a long-session quality evaluation.
- Missing-model and corrupt-model CLI runs exited successfully and matched the
  raw CTC TSV byte for byte. Neither downloaded nor retained failed-ASR audio.
- Unit checks reject expired queued results, late sends, errors and disconnected
  replies. Cleanup-order tests prove S1-mini sees canonical spelling, its output
  is corrected again, and failure keeps spelled raw text plus original recovery
  text. These use a fake cleaner, not a new S1-mini quality measurement.

Builds/inference used `systemd-run --user --scope -p MemoryMax=12G nice -n 10 env
CARGO_BUILD_JOBS=4`. Models were copied; output stayed under the worktree target.
No desktop input, microphone capture, subagents or app installation for this PR.

## Reproduce and evidence

Raw [bake-off](../Benchmarks/Dictionary/ctc-20261104/bakeoff.json),
[bounded pipeline](../Benchmarks/Dictionary/ctc-20261104/bounded.json), and
[scored report with hashes](../Benchmarks/Dictionary/ctc-20261104/report.json).
The measurement executables were built from the initial sidecar commit; subsequent
changes centralize runtime spelling and diagnostics without changing the decoder,
adoption rules or deadline. Fresh source checks are reported on the PR.

From CrossPlatform, build both release examples using the resource wrapper:

```sh
systemd-run --user --scope -p MemoryMax=12G nice -n 10 env CARGO_BUILD_JOBS=4 cargo build --release --locked --example ctc_dictionary_bench --example ctc_dictionary_deadline_bench
```

From the repository root, with copied model folders and the existing clip cache:

```sh
systemd-run --user --scope -p MemoryMax=12G nice -n 10 env CARGO_BUILD_JOBS=4 XDG_CACHE_HOME="$PWD/CrossPlatform/target/ctc-benchmark-cache" CrossPlatform/target/release/examples/ctc_dictionary_bench CTC_COPY V2_COPY Benchmarks/Dictionary/corpus.json CLIPS bakeoff.json
systemd-run --user --scope -p MemoryMax=12G nice -n 10 env CARGO_BUILD_JOBS=4 XDG_CACHE_HOME="$PWD/CrossPlatform/target/ctc-benchmark-cache" KOETT_CTC_BIAS_MODEL=V2_COPY CrossPlatform/target/release/examples/ctc_dictionary_deadline_bench CTC_COPY Benchmarks/Dictionary/corpus.json CLIPS bounded.json
python3 Benchmarks/Dictionary/ctc_report.py bakeoff.json bounded.json CLIPS report.json
```

Use a private XDG_CACHE_HOME for derived vocabulary. The existing clip synthesis
script is optional and sends public test sentences to TTS; it was not run here.

Primary evidence: [upstream transducer-only hotword support](https://k2-fsa.github.io/sherpa/onnx/hotwords/index.html)
and the [pinned C API](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.5/sherpa-onnx/c-api/c-api.h).
No CTC logits API or spelling-only operation is presented as acoustic biasing.

## Final source gates

After rebasing on main with #61 and #64: cargo fmt --check, cargo test --locked
(380 passed, 17 ignored, plus the separate native ABI check), Clippy with warnings
as errors, release build and git diff --check passed. The Windows MSVC cross-check
was attempted but blocked by missing lib.exe/native build tools on Linux.
Independent review and real microphone/long-session acceptance remain pending.

## Scaling the budget with audio length

Measured 2026-11-05 with `ctc_dictionary_budget_bench`, Ryzen 5 9600X, two-thread primary
and four-thread sidecar. Each length is 12 dictations built from corpus clips, one voice
each, with spelling applied, and the sidecar idle before each call. Raw is the same audio
with no dictionary. Added wait compares each dictation with its own raw decode.

| Length | Raw recall | Fixed 250 ms | Scaled | Scaled added wait, median (worst) | % of length |
|---|---:|---:|---:|---:|---:|
| 4 s | 4 / 24 | 12 | 16 | 238 ms (276) | 6.0 |
| 6 s | 6 / 30 | 8 | 19 | 279 ms (321) | 4.6 |
| 8 s | 11 / 43 | 11 | 26 | 370 ms (431) | 4.6 |
| 10 s | 12 / 49 | 12 | 29 | 443 ms (515) | 4.4 |
| 15 s | 21 / 75 | 21 | 46 | 646 ms (705) | 4.3 |
| 20 s | 30 / 95 | 60 | 60 | 723 ms (840) | 3.6 |
| 40 s | 57 / 188 | 123 | 123 | 1495 ms (1578) | 3.7 |
| 60 s | 89 / 284 | 184 | 184 | 2234 ms (2343) | 3.7 |

From about 6 s to 15 s the fixed budget paid its whole wait and gained nothing. Dictations
of 20 s and over are split at pauses, so they were already covered and did not change. Dictionary
terms in negative speech stayed at 0, apart from one at 60 s under both budgets.

The bar was set before measuring: added wait at most 5% of the length and at most 1 s per
decode unit. It holds from 6 s up. At 4 s it is 6.0%, against 5.9% with the fixed budget, so
this change did not cause it. Across a 60 s dictation the units add up to about 2.2 s.

On one CPU core the sidecar misses every budget up to at least 8 s: recall equals raw, as a
fallback should, and the wait is 462 to 629 ms median, 674 ms worst, for no gain. The old
budget would have cost those machines about 250 ms. That run was stopped after 8 s by its
time limit. This is a real cost on very slow machines, and only for users with 110M, a
dictionary and v2 downloaded.

All of this uses synthetic TTS voices, not microphone speech. Raw data is in
[fixed](../Benchmarks/Dictionary/ctc-budget-20261105/fixed.json) and
[scaled](../Benchmarks/Dictionary/ctc-budget-20261105/scaled.json).
