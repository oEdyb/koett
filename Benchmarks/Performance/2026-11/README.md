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
