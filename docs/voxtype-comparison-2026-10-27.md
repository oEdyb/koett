# Koett and Voxtype Linux comparison — 2026-10-27

Olle requested a comparison after unreliable recognition and desktop friction.
This is source/configuration research, not a recognition benchmark. No private
audio or transcripts were accessed and no model/default was changed.

## Versions

Koett product source: d350c3f1e583b5abcd3c36857498f33671db1f2f.
Installed package: voxtype-bin 1.1.1-1; running binary resolves to
`/usr/lib/voxtype/voxtype-vulkan`. Reviewed release source:
[v1.1.1/014c561](https://github.com/peteonrails/voxtype/tree/014c56105b2f071bb3457c51a95c38be380e5124).
Reviewed upstream dev: dc499ad47444ba6ea80d8750aa3581c23120821c.
Its src/, Cargo manifest and lockfile match the reviewed release; package metadata
alone does not prove byte-for-byte source provenance of the installed binary.

## Recognition model: strongest accuracy hypothesis

Koett's Linux default is Parakeet 110M English INT8 CTC through sherpa-onnx CPU,
two threads. Its loader also supports transducer model directories. Mac's model
and decoder are different; the Linux port does not match that model choice.
See `CrossPlatform/src/transcription.rs`, `src/runtime.rs` and the model manifest.

Olle's Voxtype selects Whisper large-v3-turbo, English, local whisper.cpp.
Turbo has about 809M parameters ([official model table](https://github.com/openai/whisper/blob/86098128c0b4f24f0e2aa2994de830614b474227/README.md#available-models-and-languages)).
The running Vulkan binary plus NVIDIA environment selects an acceleration path;
this is not a measured GPU inference claim. Vendor-to-device resolution can fall
back to loader selection ([engine/GPU source](https://github.com/peteonrails/voxtype/blob/014c56105b2f071bb3457c51a95c38be380e5124/src/transcribe/whisper.rs#L40-L104)).

Model families, capacity and decoding plausibly explain words/punctuation.
Parameter count alone does not establish accuracy, and GPU acceleration primarily
changes performance. The earlier 600M Parakeet v2 trial establishes compatibility
and resource cost, not improved personal accuracy.

## Long recordings: both process audio during capture

Koett decodes whole utterances through 20 seconds. Longer recordings use Silero VAD
speech segments in the background. Bounded decoding uses 20-second windows with
2-second overlap and text merging; that overlap is within bounded segments,
not necessarily between successive VAD segments. Background failures fall back
to decoding the retained whole recording. See `src/transcription/background.rs`.

Voxtype's configured five-second eager chunks move boundaries toward quiet
stretches. The configured 0.5-second overlap is accepted but unused in v1.1.1:
it is a deprecated compatibility field; chunks are disjoint
([setting contract](https://github.com/peteonrails/voxtype/blob/014c56105b2f071bb3457c51a95c38be380e5124/src/config/whisper.rs#L99-L113),
[chunking](https://github.com/peteonrails/voxtype/blob/014c56105b2f071bb3457c51a95c38be380e5124/src/eager.rs#L101-L161)).
Its optional VAD speech-rejection gate is not enabled in Olle's configuration;
quiet-point chunking still operates. Neither boundary strategy is automatically
more accurate. Investigate missing/repeated boundary words only with evidence.

## Capture: no demonstrated resampling defect

Both use CPAL, native input formats, mono mixing and rate conversion.
Voxtype uses stateful Rubato FFT resampling; Koett uses sherpa's stateful
resampler and flushes its tail. The name LinearResampler does not establish
missing anti-alias filtering: sherpa specifies filter cutoff and width
([filter contract](https://github.com/k2-fsa/sherpa-onnx/blob/3dc7c569f31ca2cd4a20ed6f7db780327e6714c5/sherpa-onnx/csrc/resample.h#L34-L68)).
Koett's ring buffer keeps collection off its audio callback; overflow and
incomplete capture fail explicitly. Capture quality has not been ranked.

## Paste and desktop handling: concrete friction differences

Installed Voxtype uses configurable Ctrl+Shift+V, clipboard/driver fallbacks,
pre-paste timing and optional modifier-release protection
([paste timing](https://github.com/peteonrails/voxtype/blob/014c56105b2f071bb3457c51a95c38be380e5124/src/output/paste.rs#L894-L907),
[modifier handling](https://github.com/peteonrails/voxtype/blob/014c56105b2f071bb3457c51a95c38be380e5124/src/output/mod.rs#L495-L522)).
Koett sends Ctrl+V through its portal path and Shift+Insert through its external
fallback. Terminal behavior can differ by initialized path. Koett checks the
original Niri target; non-Niri target checks are weaker. Key injection alone
never proves that an application consumed the text.

Koett caches completed text, attempts delivery and independently persists
resilient searchable history. Failed insertion does not discard recognition.
Media pause ownership also checks player identity/state and observable user
intervention; reviewed Voxtype resume simply iterates saved player names
([Voxtype resume](https://github.com/peteonrails/voxtype/blob/014c56105b2f071bb3457c51a95c38be380e5124/src/audio/media.rs#L91-L116)).
Preserve those Koett safeguards rather than replacing its whole pipeline.

## Priorities

1. Controlled shared-audio model comparison before changing the default engine.
2. Consistent paste behavior across terminal/GUI and portal/fallback paths.
3. VAD/overlap investigation if errors cluster around long-recording boundaries.

The requested Pause transfer is complete, separately verified in private Niri.
The previous listener was Olle's custom helper, not upstream Voxtype's built-in
hotkey mechanism. Its local boolean could diverge from daemon state; the new
compositor binding sends Koett a stateless toggle. See the Linux verification
report and pause-shortcut receipt. This review authorizes no additional features.
