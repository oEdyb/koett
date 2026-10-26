# Third-party notices

Koett uses the Rust crates listed in `Cargo.lock`. Each crate keeps the license
declared in its package metadata and source repository.

The local speech engine uses
[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx), which is licensed under
Apache-2.0 and uses ONNX Runtime under the MIT License.

On first use, Koett downloads NVIDIA's English Parakeet TDT-CTC 110M INT8
model from the official sherpa-onnx model release. The model is licensed under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Koett does not bundle
the model in its app archive.

Koett also downloads the 16 kHz
[Silero VAD](https://github.com/snakers4/silero-vad) model maintained by
sherpa-onnx. Silero VAD is licensed under the MIT License. Koett uses it only
to split long recordings into small speech segments before local
transcription.
