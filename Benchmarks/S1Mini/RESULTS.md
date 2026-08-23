# S1-mini results — 2026-08-22

Test machine: Apple M5 MacBook Air, 25 GB memory, macOS 26.5.2.

Versions:

- S1-mini model revision `ae016a2cc6e42d0298932503b0874f048a947f40`
- S1-mini GGUF revision `ee2c0f56e56345f475749a44ff2893e21c3cb292`
- llama.cpp `0.2.0`, build `10566`, commit `bb4caa754`

## Decision

Do not put S1-mini in Koett's normal dictation path yet. Raw Parakeet stays the
default.

S1-mini removes fillers and adds useful formatting. It also made meaning and
name changes that a dictation app cannot safely hide:

- `option once` became `Option 1` or `Option-1` in some contexts.
- `I think it was called` disappeared from a real 4:39 transcript and a focused
  test.
- `FluidAudio` became `Fluid Audio`.

Q5, Q6, Q8, F16, and all four official style settings still removed the
uncertainty phrase. The error comes from the model, not Q4 compression.

## Current Koett speed

Parakeet transcribed Olle's 279.252-second recording in 0.687 seconds after
prewarm. That is 406.2 times real time.

The existing 10-minute LibriSpeech checks took 1.736 seconds for 613.060
seconds of clean audio and 1.695 seconds for 602.910 seconds of harder audio at
four-chunk concurrency. Their normalized WER was 2.249% and 5.020%.

## S1-mini speed

The fastest safe runtime configuration was:

```text
Q4_K_M, Metal, Flash Attention on, 1 CPU thread, poll 0
3072-token context, 1024 batch, 1024 ubatch, prompt cache on
ngram-simple speculative decoding with N=7 and M=48
```

The n-gram setting produced byte-identical output to normal greedy decoding in
all 60 paired runs. It changed latency as follows:

| Input | Normal | N-gram | Speedup |
|---|---:|---:|---:|
| 50 words | 319.9 ms | 150.0 ms | 2.13x |
| 100 words | 590.5 ms | 217.6 ms | 2.71x |
| 200 words | 1,155.5 ms | 691.0 ms | 1.67x |
| 350 words | 2,223.3 ms | 1,319.6 ms | 1.68x |
| 4:39 app transcript | 3,739.4 ms | 2,089.0 ms | 1.79x |
| 4:39 direct-file transcript | 3,909.5 ms | 2,283.6 ms | 1.71x |

N-gram lookup did not help every short result. A short email-formatting case
rose from 140.8 ms to 177.1 ms. The current llama.cpp request schema does not
let a client switch n-gram lookup on for one request and off for another.

The selected 3072-token context used 0.949 GB after load. The official Q4 file
is 484,219,808 bytes. Cold server load was about 0.4 seconds.

## Quantization and runtime comparison

| Model | File size | Metal generation |
|---|---:|---:|
| Q4_K_M | 462 MiB | about 223 tokens/s |
| Q5_K_M | 526 MiB | 206 tokens/s |
| Q6_K | 594 MiB | 189 tokens/s |
| Q8_0 | 767 MiB | 161 tokens/s |
| F16 | 1.4 GiB | about 83 tokens/s |

Q4 on Metal was about 1.7 times faster than Q4 on CPU. The official BF16 model
through MLX took about 8.15 seconds for the 4:39 transcript, so llama.cpp was
the better runtime on this Mac.

## Limits

The official model card recommends less than about 1,000 input tokens. A long
recording needs transcript chunks. Chunk boundaries can change formatting and
meaning, so chunking needs its own quality gate before integration.

Sources: [S1-mini model card](https://huggingface.co/superwhisper/s1-mini),
[official GGUF files](https://huggingface.co/superwhisper/s1-mini-GGUF), and
[llama.cpp speculative decoding](https://github.com/ggml-org/llama.cpp/blob/master/docs/speculative.md).
