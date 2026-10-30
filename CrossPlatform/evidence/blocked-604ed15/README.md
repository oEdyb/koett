# Blocked cross-platform control artifacts

These reports freeze the unpublished artifacts from commit `604ed15` and CI run
[`32824639724`](https://github.com/oEdyb/koett/actions/runs/32824639724).
They are controls for the ASR-only sherpa build. They are not release files.

Reproduce all three reports from the repository root:

```sh
python3 CrossPlatform/scripts/inspect_blocked_artifacts.py \
  --control CrossPlatform/evidence/blocked-604ed15/control.json \
  --output CrossPlatform/evidence/blocked-604ed15
```

Run the inspection tests:

```sh
python3 CrossPlatform/scripts/test_inspect_blocked_artifacts.py
```

The inspector verifies the frozen CI logs, GitHub artifact SHA-256, packaged
archive, final binary, exact `sherpa-onnx-sys` crate, and the upstream native
archives before it reads imports, format-specific headers, exported names, and
printable ASCII and UTF-16LE strings. The reports combine those results with the
verified `sherpa-onnx-sys` 1.13.5 static link list. This matters because a
statically linked library does not appear as an imported runtime dependency.

The inspector is intentionally tied to the exact Python and `objdump` binaries
in `control.json`. A tool update stops the run so a reviewer can make a new
control. `ci-output.json` freezes the tested CI claims. `sherpa-inputs.json`
freezes the source and native archive evidence. `mac-model-cache.json` freezes
the per-file identity of the two compiled FluidAudio model caches found on the
control Mac. It does not replace a future release model manifest.

The control is blocked because all three binaries contain eSpeak, Piper, or TTS
evidence. The Windows binary also exports callable eSpeak and Piper symbols.

## References used

- [sherpa-onnx 1.13.5 source](https://github.com/k2-fsa/sherpa-onnx/tree/v1.13.5)
- [`sherpa-onnx-sys` 1.13.5 crate](https://crates.io/crates/sherpa-onnx-sys/1.13.5)
- [sherpa-onnx CMake TTS switch and no-TTS static package](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.5/CMakeLists.txt)
- [Cargo native linking from build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-link-lib)
- [LLVM object-file inspection](https://llvm.org/docs/CommandGuide/llvm-objdump.html)
- [GNU object-file inspection](https://sourceware.org/binutils/docs/binutils/objdump.html)
- [GitHub workflow-run log API](https://docs.github.com/en/rest/actions/workflow-runs#download-workflow-run-logs)
- [GitHub Git-reference API](https://docs.github.com/en/rest/git/refs#get-a-reference)
- [GitHub release-asset API](https://docs.github.com/en/rest/releases/assets#get-a-release-asset)
- [GitHub artifact digests](https://github.com/actions/upload-artifact/blob/main/README.md#outputs)
- [FluidAudio 0.15.6 source revision](https://github.com/FluidInference/FluidAudio/tree/4dbf4f9f9a5ff3a53ade848d7ba4e3df13db859b)
- [FluidAudio Parakeet v2 model revision](https://huggingface.co/FluidInference/parakeet-tdt-0.6b-v2-coreml/tree/ee09c569f73759e6d44c9bd16766f477b2b36d39)
- [FluidAudio Nemotron model revision](https://huggingface.co/FluidInference/nemotron-speech-streaming-en-0.6b-coreml/tree/e673531caa6d25ab7baf5a8c14c9b99ba1551838)

No official Koett-style report generator exists. This is the smallest direct
inspector built from the documented artifact, object-file, and hash interfaces.
