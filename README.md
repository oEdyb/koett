# Koett

> **Koe** (`声` / `こえ`, “voice”) → **TT** (“to text”)
>
> Open voice-to-text, on your device.

Koett is a barebones local voice-to-text app for macOS. Press one shortcut, speak,
press it again, and the transcript appears in the focused app.

Koett is early software. It currently supports English on Apple Silicon and installs
from source.

## Install

Requirements: macOS 15 or newer and the Xcode command-line tools.

```sh
git clone https://github.com/oEdyb/koett.git
cd koett
./install-macos.sh
```

The first run downloads the local Parakeet v2 model. Allow Koett to use the
microphone and Accessibility when macOS asks. The installer also starts Koett at
login.

For repeated development builds, create one Apple Development certificate in
Xcode under **Settings → Accounts → Manage Certificates**. The installer finds it
automatically. Without one, it uses an ad-hoc signature and macOS can ask for
privacy approval again after the app changes.

## Use

Toggle is the default mode:

1. Press either Option key.
2. Speak after the Tink sound.
3. Press Option again.
4. Koett transcribes locally and pastes at the cursor.

While Koett records, a small bottom-center pill shows the real microphone level
and an `m:ss` timer. The panel does not take keyboard focus.

Use the waveform menu to select Parakeet v2 or the experimental Nemotron 560 ms
model, choose Toggle or Hold, and change the shortcut. A model change is saved
and restarts Koett once. Parakeet is the default. Nemotron mode also keeps
Parakeet loaded for recovery if its live stream fails.

Koett saves every non-empty final transcript in one local Markdown file. Select
**Open Transcripts** in the waveform menu to read it. The file is at
`~/Library/Application Support/Koett/Transcripts.md`.

The final text also stays on the clipboard as a backup. Koett deletes the temporary
audio after transcription. It has no account, telemetry, or cloud transcription.

## Run without installing

```sh
swift run koett
```

Allow microphone and Accessibility access when macOS asks. Press Control-C to quit.

## Development tools

To record five seconds from the microphone and inspect the audio path:

```sh
swift run audio-microscope
```

The temporary WAV is written to `/tmp/local-voice-input-microscope.wav`.

To run Apple's on-device model against existing WAV files on macOS 26:

```sh
swift run apple-speech-baseline path/to/audio.wav
```

To run Parakeet v2 against existing WAV files:

```sh
swift run parakeet-baseline path/to/audio.wav
```

Add `--prewarm` to run one discarded full inference before the measured files.
Koett does this during startup so the first real dictation does not pay the first
inference cost.

The fixed-corpus outputs are in `Benchmarks/results.tsv`.

For a licensed long-audio speed test, download a LibriSpeech split and build one
continuous fixture at an utterance boundary:

```bash
swift Benchmarks/build-librispeech-fixture.swift \
  /path/to/LibriSpeech/test-clean /tmp/librispeech-clean-10m.wav 600
swift run -c release parakeet-baseline --prewarm --concurrency 4 \
  --transcript-directory /tmp/koett-transcripts \
  /tmp/librispeech-clean-10m.wav
python3 Benchmarks/word-error-rate.py \
  /tmp/librispeech-clean-10m.txt \
  /tmp/koett-transcripts/librispeech-clean-10m.txt
```

The fixture builder also writes the reference transcript and a source manifest
beside the WAV file. LibriSpeech is licensed under CC BY 4.0.
The measured M5 results are in `Benchmarks/long-results.tsv`.

To compare cache-aware streaming, run the same 16 kHz mono fixture through
Nemotron. The fixture builder produces this format. Use `--realtime` for a paced
microphone-like run. Without it, the command measures uncapped streaming
throughput and the final flush after simulated key release.

```bash
swift run -c release nemotron-streaming-baseline --chunk 560 \
  --feed-ms 20 --transcript-directory /tmp/koett-nemotron \
  /tmp/librispeech-clean-10m.wav
```

The first run downloads about 600 MB for each selected Nemotron chunk size.

To force the experimental live adapter for one command-line launch:

```bash
swift run -c release koett --nemotron
```

This records a temporary Parakeet recovery WAV while `AVAudioEngine` feeds ordered
microphone audio to the Nemotron 560 ms model. The live stream is bounded to ten
minutes. The waveform menu provides the same model choice and saves it between
launches.

## Record one clean sentence

```sh
swift run sentence-recorder normal-002 "Write the sentence here."
```

The recorder waits for Return. One Tink sound means start. Two Basso sounds mean stop. It records
for six seconds and saves `Benchmarks/Local/normal-002.wav`.

## Make noisy test files

Record clean speech only. Download the four small public noise files, then mix one:

```sh
./Benchmarks/download-noise.sh
```

```sh
swift run noise-mixer clean.wav Benchmarks/Noise/Typing_1.wav Benchmarks/Mixed
```

The command creates light (`20 dB`), medium (`10 dB`), and heavy (`0 dB`) noise
versions. It uses a fixed start point, so every model receives the same audio.
Keep each noise file's source, license, and checksum in
`Benchmarks/noise-sources.tsv`.

## Verified versions — 2026-08-09

- macOS 26.5.2, Xcode 26.6, macOS SDK 26.5, and Swift 6.3.3.
- FluidAudio 0.15.5, exact revision `19600a485baa4998812e4654b70d2bab8f2c9949`.
- Parakeet v2 Core ML repository revision `ee09c569f73759e6d44c9bd16766f477b2b36d39`.
- Local 450 MB Parakeet model tree SHA-256 `6b4a42f58e6ec3f7911f5889968992d73662789c3624d82947bd30f4ac9123b4`.
- Apple Speech framework interface version 3525.2.2. Apple manages its model and does not expose a model revision.

API references: [Apple SpeechAnalyzer](https://developer.apple.com/documentation/speech/speechanalyzer),
[Apple AssetInventory](https://developer.apple.com/documentation/speech/assetinventory),
[Apple NSEvent global monitor](https://developer.apple.com/documentation/appkit/nsevent/addglobalmonitorforevents(matching:handler:)),
[Apple AVAudioRecorder](https://developer.apple.com/documentation/avfaudio/avaudiorecorder),
[Apple AVAudioRecorder metering](https://developer.apple.com/documentation/avfaudio/avaudiorecorder/ismeteringenabled),
[Apple AVAudioEngine](https://developer.apple.com/documentation/avfaudio/avaudioengine),
[Apple AVAudioEngine input node](https://developer.apple.com/documentation/avfaudio/avaudioengine/inputnode),
[Apple AVAudioNode recording tap](https://developer.apple.com/documentation/avfaudio/avaudionode/installtap(onbus:buffersize:format:block:)),
[Apple NSWorkspace.OpenConfiguration](https://developer.apple.com/documentation/appkit/nsworkspace/openconfiguration),
[Apple applicationSupportDirectory](https://developer.apple.com/documentation/foundation/url/applicationsupportdirectory),
[Apple FileHandle](https://developer.apple.com/documentation/foundation/filehandle),
[Apple NSWorkspace.open](https://developer.apple.com/documentation/appkit/nsworkspace/open(_:)),
[Apple NSPanel](https://developer.apple.com/documentation/appkit/nspanel),
[Apple Timer](https://developer.apple.com/documentation/foundation/timer),
[Apple UserDefaults](https://developer.apple.com/documentation/foundation/userdefaults),
[Apple NSPasteboard](https://developer.apple.com/documentation/appkit/nspasteboard),
[Apple CGEvent](https://developer.apple.com/documentation/coregraphics/cgevent),
[Apple CGEvent.post](https://developer.apple.com/documentation/coregraphics/cgevent/post(tap:)),
[Apple SMAppService](https://developer.apple.com/documentation/servicemanagement/smappservice),
[Apple LSUIElement](https://developer.apple.com/documentation/bundleresources/information-property-list/lsuielement),
[Apple NSMicrophoneUsageDescription](https://developer.apple.com/documentation/bundleresources/information-property-list/nsmicrophoneusagedescription),
[FluidAudio 0.15.5](https://github.com/FluidInference/FluidAudio/releases/tag/v0.15.5),
[FluidAudio Nemotron](https://github.com/FluidInference/FluidAudio/blob/19600a485baa4998812e4654b70d2bab8f2c9949/Documentation/ASR/Nemotron.md), and
[Parakeet v2 Core ML](https://huggingface.co/FluidInference/parakeet-tdt-0.6b-v2-coreml).

Noise data reference: [Microsoft MS-SNSD](https://github.com/microsoft/MS-SNSD).
Its noise set combines CC0 Freesound files and CC BY-SA 3.0 DEMAND files. The local
manifest conservatively records CC BY-SA 3.0 because MS-SNSD does not identify the
source license of each selected file.

## Current limits

- English only.
- Apple Silicon only.
- No signed download yet.
- No speech-evidence gate yet.
- Overlapping speakers can produce incorrect text.

## License

Koett is licensed under [Apache License 2.0](LICENSE). FluidAudio is Apache 2.0.
The downloaded Parakeet v2 model is CC BY 4.0.
