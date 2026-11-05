# Koett sherpa-onnx wrapper patch

Original crate: sherpa-onnx 1.13.5, SHA-256
`352a5dbbd9623e800be27e77de514e87fcfe2120256acc4434e6ea87a56bc885`.
The original Apache-2.0 LICENSE, manifest, README and source files are retained.
`koett-provenance.json` records original file digests and the patched module digest.

Only `src/lib.rs` changes: it excludes the TTS module and public TTS re-export,
removes that unavailable API's example/list entry and marks the modification.
It also re-exports the verified shared-context capability and patched native
source fingerprint from sherpa-onnx-sys.
ASR, streaming, VAD, resampling, WAV I/O and other upstream implementations are
unchanged. The original README describes the full upstream crate; Koett's native
runtime and this wrapper omit sherpa TTS. Optional spoken Ask uses its separate
implementation.

Windows debug linking can retain an object containing unused Rust TTS calls;
those references cannot resolve against an ASR-only native library. Compile-time
module exclusion aligns both layers without restoring native TTS stubs, importing
TTS libraries or suppressing linker errors. The pinned source/target receipt guard
and TTS/Piper/eSpeak exclusions remain unchanged.

The separately patched native NeMo header adds the explicit koett_context mode:
one encoder feeds a greedy base decoder and a biased beam decoder. Empty
context streams use greedy decoding, so dictionary changes keep the loaded model. Other native
decoding modes retain upstream behavior. The portable patch and build recipe are
provided in package sources/. Patched header SHA-256:
1200f3e6b686eb3a47cc5982c906ba0b0f0b7f9b0e3e69d987556c85c00137f3.
