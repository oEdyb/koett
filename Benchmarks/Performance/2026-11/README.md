# Performance receipts

`linux-asr.json` is the original before-fix file-ASR probe. It does not time
capture or paste. `linux-model-load.json` is the earlier shared candidate's
warm-filesystem load repeat; it is not the final model-reuse build.

The final dictionary CPU plot is generated from
`Benchmarks/Dictionary/2026-11/model-reuse/context.json` by `plot.py`.
Run it with Python and Matplotlib. The figure measures aggregate inference wall
time on CPU, not Stop-to-paste, and states the synthetic-corpus limitation.

The workflow report identifies app hashes, endpoints, live target receipts and
which build each measurement belongs to. No live transcript-store content is
part of these exports.

`check-mac.py` independently recomputes the frozen 24 native M5 target receipts
from source commit `3998053ffe7c5ee451476fcffb03cd8742796193`, plus 60 component
trials and 12 cache-ablation/production targets from
`d1b689a23b286dc5c8a40a925af6f70e88dc9c51`. It checks source
identity, clipboard generations, payload equality, final phrase/length guards,
ordered captures and every timing without printing payloads. Fetch the source
objects if absent locally:

```sh
git fetch origin refs/pull/105/head refs/pull/110/head
python3 Benchmarks/Performance/2026-11/check-mac.py
```

The original
synthetic outputs and reference-aware verifier remain in that audit's
`Benchmarks/Dictionary/PasteAudit/Host/`; physical microphone/key latency is
outside this controlled-input comparison.

The cache probe and final production arm are separate measurements. The latter
has only three 5 s and one 120 s targets, with no repeated 30/600 s matrix. Its
ASR phase also rises systematically relative to the baseline; the cause is
unexplained. The data proves removal of the expensive automatic menu lookup,
not a universal end-to-end speedup or restored parity with v0.1.1.
