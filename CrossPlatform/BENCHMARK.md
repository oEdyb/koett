# Mac spike result

Measured on 2026-08-13 on Olle's M5 MacBook Air with macOS 26.5.2. Both
models used the same `sherpa-onnx` 1.13.5 CPU path with two threads. The model
loaded once before the eleven registered local Koett corpus files ran.

| Model | Model files | Normalized exact files | Normalized WER | Mean transcription time | Peak RSS |
|---|---:|---:|---:|---:|---:|
| Parakeet TDT-CTC 110M INT8 | 126 MB | 9/11 | 5.405% | 131.7 ms | 407 MB |
| Parakeet TDT 0.6B v3 INT8 | about 616 MB | 10/11 | 2.703% | 645.1 ms | 1.19 GB |

Both models returned empty text for the silence fixture. The 110M model missed
technical terms in two files. It stays the lightweight spike candidate, not the
shipping default, until a larger corpus and real Windows and Linux results pass.

The statically linked macOS release executable was 25 MB. Model files are not
inside the executable.

The default microphone path captured 48 kHz Float32 samples through CPAL without
overflow. A quiet-room run returned empty text. The live path still needs a
spoken hardware test on Windows and Linux.
