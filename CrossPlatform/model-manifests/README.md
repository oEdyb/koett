# Model manifest

Koett embeds one reviewed JSON manifest for its default Rust model cache. The
download URL is only a location. The exact byte count and SHA-256 are the trust
boundary because the upstream `asr-models` GitHub release is not immutable.

The manifest records both downloaded artifacts and every file required at
runtime, including the two self-test WAV files. Koett verifies an artifact
before extraction. It then assembles and verifies the full ASR and VAD cache in
a unique transaction directory. One directory rename makes the complete cache
live. Concurrent installers keep the first valid cache and discard their own
private transaction.

After a full installed-file check, Koett stores a small local verification
stamp with the embedded manifest hash and each file's byte count and modification
time. An unchanged cache can start without hashing 132 MB again. A manifest,
size, or modification-time change forces a full SHA-256 check. A model-load
failure also bypasses the stamp and runs the full check. A future signed Koett
release will cover the embedded manifest through the application signature; the
current local cross-platform branch remains unsigned.

The upstream sherpa-onnx documentation does not publish the source commit that
produced either converted release asset. The `upstream_reference_url` fields are
license and attribution references, not build-provenance claims. Koett therefore
identifies each downloaded artifact by its GitHub asset ID, exact byte count, and
content hash.

Primary references:

- [sherpa-onnx model instructions](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-ctc/nemo/english.html#sherpa-onnx-nemo-parakeet-tdt-ctc-110m-en-36000-int8-english)
- [NVIDIA Parakeet model card and CC-BY-4.0 license](https://huggingface.co/nvidia/parakeet-tdt_ctc-110m/tree/431a349f3051ab85c22b9b7a2741b5fe77065665)
- [Silero VAD source and MIT license](https://github.com/snakers4/silero-vad)
- [GitHub release API](https://api.github.com/repos/k2-fsa/sherpa-onnx/releases/tags/asr-models)
- [`std::fs::rename`](https://doc.rust-lang.org/1.98.0/std/fs/fn.rename.html)
- [`std::fs::create_dir`](https://doc.rust-lang.org/1.98.0/std/fs/fn.create_dir.html)
- [`AtomicU64::fetch_add`](https://doc.rust-lang.org/1.98.0/std/sync/atomic/struct.AtomicU64.html#method.fetch_add)
- [`File::sync_all`](https://doc.rust-lang.org/1.98.0/std/fs/struct.File.html#method.sync_all)
- [`sha2` 0.11.0](https://docs.rs/sha2/0.11.0/sha2/)
- [`serde_json` 1.0.151](https://docs.rs/serde_json/1.0.151/serde_json/)

There is no upstream sherpa-onnx cache manifest or recovery implementation that
covers Koett's app-owned model directory. This is the smallest direct solution
using Koett's existing `sha2`, `serde`, and `serde_json` dependencies.
