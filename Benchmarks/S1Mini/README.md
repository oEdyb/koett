# S1-mini benchmark

This harness measures S1-mini after Koett's raw Parakeet transcript. It does not
change or install the app.

It follows the official prompt, control line, thinking-off setting, greedy
decoding, input-sized output limit, and roughly 1,000-token input limit:

- https://huggingface.co/superwhisper/s1-mini
- https://huggingface.co/superwhisper/s1-mini-GGUF

The tested model revision is
`ee2c0f56e56345f475749a44ff2893e21c3cb292`. Install `llama.cpp`, download one
official GGUF file outside the repository, then run:

```sh
python3 Benchmarks/S1Mini/benchmark.py \
  --model /path/to/s1-mini-q4_k_m.gguf \
  --label q4 \
  --runtime metal \
  --repeats 3
```

Use `--spec ngram-simple` to test lossless prompt-lookup speculative decoding.
The harness fails with status `2` when a meaning or exact-output check fails.

The best measured settings on the M5 test Mac were:

```sh
python3 Benchmarks/S1Mini/benchmark.py \
  --model /path/to/s1-mini-q4_k_m.gguf \
  --label q4 \
  --runtime metal \
  --spec ngram-simple \
  --ngram-n 7 \
  --ngram-m 48 \
  --threads 1 \
  --poll 0 \
  --context 3072 \
  --batch-size 1024 \
  --ubatch-size 1024 \
  --repeats 3
```

See [RESULTS.md](RESULTS.md) for the measured result and integration decision.
Raw JSON reports stay local under `Results/`.
