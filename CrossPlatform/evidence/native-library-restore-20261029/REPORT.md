# Native library recovery

Restored the pinned sherpa-onnx 1.13.5 ASR-only static artifacts to `/home/olle/.cache/koett/native/sherpa-onnx-1.13.5-linux-abi0-floor/lib`; `/tmp/koett-native-host-abi0` is a symlink to that disk-backed root.

All 10 archives and the ASR marker match the current-PC repository cache and independent Docker floor volume byte-for-byte by SHA256. `receipt.json` lists exact hashes, source commit/archive identity and ABI0/TTS-disabled options. The deleted host cache has no surviving hash receipt, so this is compatible verified-cache recovery, not a claimed exact restoration of that lost host build.

The floor source archive SHA256 matches the pinned recipe; its source declares version 1.13.5. The native runtime probe links all archives and returns 1.13.5. Its Git string `17ab05b7` is hardcoded upstream in the verified archive; full source provenance is commit `3dc7c569f31ca2cd4a20ed6f7db780327e6714c5`. The C API exports ASR recognizer creation; defined-symbol scans find no blocked TTS/espeak/piper/UCD symbols.

The actual linked focused Rust library test passed: 1 test, 149 filtered. See `focused-linked-test.log` and `native-boundary.log`. No model inference, audio device or GUI was exercised.

Stateful-menu source checks were already green; final linked release and newest formatting CLI test remain pending parent execution, along with full app gates. No application/source/dependency edits were made here.
