# ASR wrapper link boundary

Windows CI's native source build and strict receipt check passed, then debug
linking retained unused safe-wrapper TTS references. The same five C externals
are present in a locally compiled Windows COFF wrapper archive before the patch.
After excluding that module, they are absent while 38 ASR externals remain.
This is object-level evidence, not a native Windows linker/runtime claim.

Only the safe wrapper module index/documentation changes. All published crate
files are retained, original file hashes are recorded, and ASR/streaming/VAD/WAV/
resampler implementations remain unchanged. Original license and modification
notices accompany both Linux and Windows packages. The strict native receipt
and native TTS exclusion are unchanged. Real public-sample 110M/v3 inference
passed with the patched wrapper, without capture or desktop interaction.

Two independent source reviews completed. The identified inventory inheritance,
Windows notice delivery and original metadata omissions were corrected, with
focused regression checks. Root Cargo.lock changes only the wrapper's registry
to local-source identity; unrelated version/edge changes were discarded.

Final source/target CI must still verify native Windows linking and release
packages. No model defaults, user preferences, installed app or Mac verification
state changed.
