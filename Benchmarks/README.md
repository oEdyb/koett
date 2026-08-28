# Koett benchmark records

Koett uses one flat run record for JSONL, JSON, and TSV. Each row is one trial.
Failed trials stay in the file. Raw ASR uses `stage=raw_asr`. Optional cleanup
uses `stage=formatting`, pins its formatter and profile, and points to its raw
input with `source_run_id`. Never replace raw text with formatted text in a
benchmark record.

`benchmark_record.py` validates records, joins them to `corpus.tsv`, scores
them, and writes the same fields in either JSONL, JSON, or TSV:

```sh
python3 Benchmarks/benchmark_record.py runs.jsonl \
  --corpus Benchmarks/corpus.tsv \
  --output scored.tsv \
  --summary summary.json
```

For a model comparison, give the two explicit configuration names. The output
reports the measured parts of Koett's model gate. It does not issue a final
promotion decision:

```sh
python3 Benchmarks/benchmark_record.py runs.jsonl \
  --corpus Benchmarks/corpus.tsv \
  --output scored.tsv \
  --summary comparison.json \
  --control parakeet-control \
  --candidate moonshine-small
```

The identity fields pin the named configuration, app, adapter, runtime, model,
decoder, normalizer, platform, hardware, power mode, target, fixture, and repeat.
A configuration name must map to one exact identity. Result fields keep explicit
success or failure details and the unchanged hypothesis. Numeric trace fields
cover the shortcut, capture, model, formatting, delivery, target mutation,
paint, and stable-text events. The record also keeps cold-load, prewarm,
engine, visible-latency, installed-footprint, memory, CPU, and energy
measurements when a harness can measure them.

The scorer reports substitutions, deletions, insertions, normalized WER and CER,
protected-term misses, first-word retention, final-word retention, expected
silence, and failures. A protected term can contain more than one word. Terms
use the same deterministic `koett-words-v1` normalization as WER. Aggregate WER
is total edits divided by total reference words. It is not the mean of per-file
WER. Latency summaries use linear p50, p95, and p99 interpolation. They include
missing counts, reject impossible causal order, and reject an explicit duration
that conflicts with its trace. Timing scope is part of the comparison identity.
Summaries keep each exact configuration separate
inside raw-ASR and formatting stages.
False pastes on expected-silence fixtures are reported separately and are not
folded into spoken-audio WER.
`safety_passed` means only that the run succeeded and kept its empty-audio,
protected-term, first-word, and final-word checks. It is not a full accuracy
pass and does not replace WER or a model promotion gate.

The optional comparison matches paired `(fixture_id, repeat)` runs. It resamples
whole speaker blocks with a fixed seed, so recordings and repeats from one
speaker stay together. The 95% interval is for candidate-minus-control WER in absolute
percentage points. The promotion result also requires complete locked coverage
for negation, number, path, and code fixtures. The current 11-file local corpus
has no path or code recording. Its files are also marked non-publishable until
that status is reviewed. The scorer requires 100 publishable recordings from 10
speakers for the minimum public sample gate. It does not invent evidence or pass
an incomplete promotion gate.

`results.tsv` and `long-results.tsv` are preserved legacy measurements. They
do not satisfy the new record contract and must not be mixed into a new claim
without an explicit adapter that supplies the missing identity, failure, and
timing fields. `word-error-rate.py` now reuses the common normalizer and
alignment code.

Run the tests with the oldest supported Python:

```sh
cd Benchmarks
python3 test_benchmark_record.py
```

This uses only Python 3.10 standard-library APIs. No exact upstream format
covers Koett's cross-platform target-visible events, protected terms, and
raw-versus-formatting split, so this is a small project-owned contract.

Primary references:

- [NIST SCTK SCLITE](https://github.com/usnistgov/SCTK/blob/master/doc/sclite.htm)
  defines minimum-edit reference/hypothesis alignment and WER scoring.
- [NIST SCTK](https://github.com/usnistgov/SCTK) is the official maintained
  speech-recognition scoring toolkit.
- [Bisani and Ney, 2004](https://www-i6.informatik.rwth-aachen.de/publications/download/401/BisaniM.NeyH.--BootstrapEstimatesforConfidenceIntervalsinASRPerformanceEvaluation--2004.pdf)
  defines bootstrap confidence intervals for ASR WER comparisons.
- [Liu and Peng, 2020](https://www.isca-archive.org/interspeech_2020/liu20c_interspeech.html)
  shows why dependent utterances should stay together in bootstrap blocks.
- [Python 3.10 `random`](https://docs.python.org/3.10/library/random.html)
  documents the seeded deterministic generator used for reproducible resampling.
- [Python 3.10 `csv`](https://docs.python.org/3.10/library/csv.html) documents
  `DictReader`, `DictWriter`, tabular dialects, and `newline=''`.
- [Python 3.10 JSON](https://docs.python.org/3.10/library/json.html) documents
  the deterministic JSON encoding used for JSON and JSONL.
- [Retrieval Augmented Correction of Named Entity Speech Recognition Errors](https://arxiv.org/abs/2409.06062)
  shows why rare entities need a separate target metric instead of WER alone.
- [Unified End-to-End Speech Recognition and Endpointing](https://arxiv.org/abs/2211.00786)
  measures endpoint latency together with WER, which supports Koett's separate
  final-word and visible-latency fields.
