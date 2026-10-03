# Microphone selection

The menu stores an explicit Core Audio device UID, or no UID for System Default.
It resolves the current device ID for each recording. An available explicit choice
stays selected. A missing choice is replaced by an available built-in microphone,
then the macOS default or first available input; the replacement is saved. Reconnecting
the old input does not take over the saved replacement. System Default remains a
mode, following the available default with fallback while that default is missing.
When no inputs exist, the menu says **None available** and recording fails visibly.
The saved choice is retained until an available replacement can be selected.
Dictation and Ask share the choice; Nemotron and its recovery WAV
receive the same resolved device. Koett does not change the macOS default input.

## Automatic availability — 2026-10-03

Startup, device/default changes, current-input alive changes, wake and app activation
refresh the selection. Core Audio devices marked not alive are excluded even if
they remain in the device list. Listeners are installed once, rolled back after
partial failures, and removed on teardown; queued callbacks from old registrations
cannot refresh the current state.

If the active capture device disappears, Koett stops it and preserves its WAV through
the existing failure path. Ask also cancels its pending screenshot and prewarmed
speech. The fallback is used for the next session; audio formats/devices are never
combined inside the interrupted recording.

Software tests cover saved fallback, available choice stability, System Default,
temporarily missing default, no-input recovery, catalog errors, actual menu state,
preserved recording frames, Ask interruption and observer lifetime. Physical USB/
AirPods disconnect/reconnect and live Nemotron checks still need a device. No new
build was installed; the September verification below describes the older install.

Sources checked: [Apple listener registration](https://developer.apple.com/documentation/coreaudio/audioobjectaddpropertylistenerblock(_:_:_:_:)), [built-in transport](https://developer.apple.com/documentation/coreaudio/kaudiodevicetransporttypebuiltin), and the current SDK's `AudioHardwareBase.h` definition of `DeviceIsAlive` (1 ready/available, 0 unusable).

Capture uses an input-only AUHAL unit. Koett selects the device before initializing
or starting the unit. AVAudioEngine initially configured the default device when
its input node was accessed, before Koett could select the built-in microphone.
A capture-only probe reproduced an AirPods output-rate change from 48 kHz to
24 kHz and back. A start-sound-only probe did not change the output rate.

The AUHAL client format matches the selected hardware rate and channel count.
Extended Audio File Services converts input to the existing 16 kHz mono Float32
WAV. Asynchronous writes are primed before recording. The callback uses a
preallocated buffer and never waits for the sink mutex; missed writes surface as
capture errors. File disposal flushes pending writes. Hardware state and format
are checked before stopping, and capture errors preserve recoverable audio.

Nemotron uses the same input-only capture component. A failed backup WAV does
not cancel a healthy primary transcript. If both fail, Koett retains the backup
and reports failure instead of transcribing incomplete audio as a successful result.

## Sources checked on 2026-09-21

- [Apple TN2091: device input using AUHAL](https://developer.apple.com/library/archive/technotes/tn2091/_index.html): device selection, input/output scopes and the requirement that client and hardware sample rates match.
- [Apple asynchronous file writes](https://developer.apple.com/documentation/audiotoolbox/extaudiofilewriteasync(_:_:_:)): priming, asynchronous conversion and final flush.
- [Apple AVAudioIONode](https://developer.apple.com/documentation/avfaudio/avaudioionode): default device behavior and access to the underlying audio unit.
- [Apple Extended Audio File client format](https://developer.apple.com/documentation/audiotoolbox/kextaudiofileproperty_clientdataformat): PCM format conversion for file writes.
- Local macOS 27 SDK headers `AudioHardwareBase.h`, `AudioUnitProperties.h`, `AVAudioEngine.h`, `AVAudioNode.h`, `ExtendedAudioFile.h`, `AVAudioRecorder.h`: UID ownership, supported device-selection scope, input-only taps and unavailable macOS recorder channel assignments.
- FluidAudio remains pinned to 0.15.6, revision `4dbf4f9f9a5ff3a53ade848d7ba4e3df13db859b`; the existing streaming adapter and model lifetime are preserved.

## Verification — 2026-09-21

On the Apple Silicon host, the final production recorder captured three successive
three-second recordings from MacBook Air Microphone while AirPods remained the
system default. Each capture reported about three seconds and no capture error.
The output-device sample-rate listener reported zero changes during capture,
stopping and recorder disposal. The previous engine-based implementation changed
the AirPods output rate to 24 kHz and back to 48 kHz in the same probe.
Test audio was deleted after inspection. The initial chooser implementation also
passed three two-second captures with exactly 32,000 Float32 mono frames at 16 kHz.
A native menu preview using the production menu builder showed System Default,
built-in, iPhone and AirPods choices with the built-in input checked.

Automated tests cover persisted choice, changed transient device IDs, missing
selected input, System Default, conversion from 48/44.1/16 kHz, stereo input,
metering, duration, invalid hardware initialization and preserving audio after a capture error. Real hardware
unplug/reconnect during recording and a live Nemotron model session remain
unverified. The live hardware test exercises capture; Olle confirmed that the corrected installed build seemed to resolve the music
pause. Full pasted-text delivery was not independently checked.

The final 76-test suite, release build, diff check, installer syntax and both
packaging plist checks passed. Independent review has no remaining findings.
The local app was installed with the existing Developer ID identity. Strict
signature verification passed; release and installed UUIDs both equal
`37ACCE69-9290-32A4-8525-FE954F74BC9D`. One installed process was running. Saved
input is `BuiltInMicrophoneDevice`; Parakeet and raw formatting remain selected.
The initial chooser build saved a fresh transcript after installation. After the
AUHAL correction was installed, Olle live-confirmed: “Hey, it seemed to work.” No source commit,
push or public release was made.
