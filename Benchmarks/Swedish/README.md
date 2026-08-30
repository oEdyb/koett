# Swedish model bakeoff

This benchmark compares Koett's current Parakeet v2, multilingual Parakeet v3,
and KBLab's quantized KB-Whisper Base on the same Swedish audio. It does not
change the app or its default model.

Prepare the exact public data, model, and runtime:

```sh
zsh Benchmarks/Swedish/prepare.sh
swift build -c release --product parakeet-baseline
```

Run one complete 758-file accuracy and resource pass:

```sh
python3 Benchmarks/Swedish/bakeoff.py \
  --parakeet .build/release/parakeet-baseline \
  --whisper "$HOME/Library/Caches/koett-benchmarks/whisper.cpp/build/bin/whisper-cli" \
  --kb-model "$HOME/Library/Caches/koett-benchmarks/kb-whisper-base-q5_0.bin" \
  --fleurs-tsv "$HOME/Library/Caches/koett-benchmarks/fleurs-sv/test.tsv" \
  --fleurs-audio "$HOME/Library/Caches/koett-benchmarks/fleurs-sv/audio/test" \
  --output Benchmarks/Swedish/Results
```

The output contains the generated corpus manifest, raw records, scored records,
summary, process receipts, logs, and `REPORT.md`. Results are ignored until a
review decides that they are suitable to publish.

The batch keeps each model loaded across all files. Parakeet exposes honest
per-file engine time. whisper.cpp does not, so KB-Whisper records accuracy only
and keeps wall time and peak RSS in its process receipt. The benchmark never
divides one batch time across files.

FLEURS test audio and text are CC BY 4.0. KB-Whisper is Apache 2.0. whisper.cpp
is MIT. The benchmark pins:

- FLEURS revision `70bb2e84b976b7e960aa89f1c648e09c59f894dd`;
- KB-Whisper Base revision `1499d2d2f0c7ed545bd6f2eec85287cf8d8c8b38` and its official Q5_0 GGML file;
- whisper.cpp v1.9.3 commit `371b5a7561823ab2bb32142d2751e35e7534727b`;
- Koett's exact FluidAudio `0.15.6` revision from `Package.resolved`.

Primary sources:

- [Google FLEURS](https://huggingface.co/datasets/google/fleurs)
- [KBLab KB-Whisper Base](https://huggingface.co/KBLab/kb-whisper-base)
- [whisper.cpp](https://github.com/ggml-org/whisper.cpp/tree/b4938)
- [FluidAudio manual Parakeet loading](https://github.com/FluidInference/FluidAudio/blob/main/Documentation/ASR/ManualModelLoading.md)
