# Koett cross-platform engine

This is the first Windows and Linux portability spike. It proves local WAV and
default-microphone transcription. It does not contain shortcuts, paste, or UI.

Build it:

```sh
cargo build --release
```

Run it with an official sherpa-onnx Parakeet INT8 model directory and one or more
mono 16-bit PCM WAV files:

```sh
./target/release/koett-engine MODEL_DIRECTORY AUDIO.wav
```

On Windows, run the matching `.exe`:

```powershell
.\target\release\koett-engine.exe MODEL_DIRECTORY AUDIO.wav
```

Record from the default microphone for five seconds and transcribe it:

```sh
./target/release/koett-engine MODEL_DIRECTORY --mic 5
```

The model comes from the official
[sherpa-onnx model page](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-ctc/nemo/english.html#sherpa-onnx-nemo-parakeet-tdt-ctc-110m-en-36000-int8-english).

The lightweight CTC directory contains `model.int8.onnx` and `tokens.txt`. A
Parakeet transducer directory contains `encoder.int8.onnx`,
`decoder.int8.onnx`, `joiner.int8.onnx`, and `tokens.txt`. The engine detects
either official layout.
