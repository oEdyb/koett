# Mac parity acceptance

As of 2026-11-04. Source implementation and live acceptance are separate.
The canonical cross-platform table remains [feature-parity.md](feature-parity.md).
Public notarized v0.1.1 remains separate from these source changes; no release
publication, default-model switch or cross-platform rewrite is included.

## Implemented

- Repeat drawing paste: one PNG/TIFF/plain-text/embedded-image HTML item;
  preserve a newer copy during either staging transition. First delivery stays
  image then text. Done/Copy/last-result paths share the same bundle.
- Optional Parakeet v3 and native 110M, using pinned FluidAudio 0.15.6. v2 stays
  default and Nemotron recovery. Relaunch preserves each selected model.
- Opt-in explicit accessible Pause controls for supported players. No blind
  media key, private MediaRemote dependency, automatic Play or new permission.
  Native players are searched before browser trees within the bounded scan.
- Original process/launch identity and available AX window checked before each
  automatic paste; held modifiers and Koett itself reject injection.
- Retry Save reuses stable recovery IDs; native login toggle preserves an
  explicitly disabled preference across reinstalls.
- Native microphone picker/meter. No preview audio storage or ASR; changing
  inputs, close and recording release its capture. Light/dark captures inspected.
- Native media file/URL entry, shared ASR/storage/recovery, private temporary
  conversion and original-file preservation. No browser-cookie import.
- Nemotron Pause/Resume drops both live and backup paused buffers without
  advancing captured-audio time or restarting the model. Undo Last Drawing
  removes pending/captured frames and rejects late results; editor undo is untouched.
- Explicit image-only pins when Include Page Details in Pins is off.
- [Local JSON commands](macos-control.md) over a bounded user-only Unix socket.
  No network listener, transcript content or keys in the control response.

## Evidence and limits

- Full suite on the final code: `swift test`, 414 tests, 10 skipped, no failures.
  Skips are not passing live acceptance. Release build and packaging checks pass.
- Eight initial clipboard tests, including delayed reads, repeated format reads,
  failed injection and newer-copy races. [Chrome acceptance](evidence/macos-parity-20261104/clipboard.md):
  two real pastes each inserted one image and exact text in an isolated rich editor.
- [Model probe](evidence/macos-parity-20261104/models.json): separate test models,
  generated English/Swedish audio. English words matched for v3/110M; v3 spelled
  Olle as Ole in Swedish. This is not a human-speech accuracy benchmark.
- Isolated microphone capture lifecycle, local FFmpeg conversion, paused ordered
  audio, pending/captured undo, socket ownership/limits and command-error tests.
- Forty pill states/morph frames rendered and inspected in light/dark; existing
  300×48 layout, timers and transitions remain unchanged. New microphone window
  captures are alongside the [pill contact sheet](evidence/macos-parity-20261104/pill-contact.png).

## Rows still open

- **Live T3 paste and full target matrix:** the preview keyboard produced no paste
  event. T3 0.1.25's source accepts this image+HTML+text contract, but source is
  not proof of its actual chat-box behavior. Native rich text/terminal keyboard
  paste and physical newer-copy races still need real target acceptance.
- **Universal paste receipt:** foreground checks reduce risk; macOS has no atomic
  “paste only into this window” operation or universal consumption receipt.
  Keep saved text and manual Copy/Paste recovery rather than claim guaranteed delivery.
- **Media controls:** real player AX trees remain unconfirmed. Unsupported or
  hidden controls, other languages and non-YouTube browser media are no-ops;
  resume manually. [API evidence and chosen limits](media-pause.md) explain why
  MediaRemote's read restriction does not prove every pause approach impossible.
- **Automatic private-window detection:** no authoritative private-mode signal is
  wired into the AX reader. Use image-only pins; this omits URL, title, selection
  and pointer context but does not remove the deliberately captured screenshot.
  A browser bridge remains a separate integration, not an inferred privacy flag.
- **Nemotron silence trim:** audio has already reached the live model before final
  ASR. Trimming the backup afterward cannot retroactively trim the stream. Empty
  results remain quiet; a validated streaming VAD/segmentation path is still needed.
- **Nemotron acoustic dictionary bias:** spelling correction remains the labeled
  alternative. No validated acoustic hook or measured rescorer is added. v3/110M
  dictionary accuracy and human speech also require their own corpus acceptance.
- **Crash during capture:** no crash-safe recording journal. Prior force-kill
  probes produced no readable WAV frames; normal Quit remains guarded. Durable
  audio retention needs an explicit bounded journal/recovery design, not a false
  claim that completed-transcript recovery covers process kills.
- **Live hardware:** 30-minute microphone endurance, real stop-to-paste latency,
  dual/mixed-DPI monitors, rotation/hotplug, physical draw-key chords, Bluetooth
  and device changes remain unmeasured. Over-15-minute URL ASR still bypasses
  the buffered trim/boost/progress path. Unit/probe output cannot substitute for these.
- **Native appearance/accessibility matrix:** new microphone and existing pill
  light/dark captures are inspected; all high-contrast/scaling/keyboard/system
  combinations and installed Ask Markdown/math remain live acceptance items.
- **Vimeo:** the known anonymous 401 is an upstream/auth boundary. User-selected
  local files are now the app alternative; cookies are never imported silently.
- **Click/spoken-cue keyframes:** no exact shipped cue behavior or acceptance spec
  exists on any platform. Explicit drawing and pins remain the available alternative.
- **Distribution:** no new public notarized release or Intel Mac artifact is
  published. Local Apple Development installation is separate from public release.

## Try the changes

1. Draw while dictating into Chrome and T3. Press Cmd+V twice: each paste should
   keep image and text. Copy another item during delivery: the newer copy should win.
2. Switch focus to another app during ASR. Expect Not pasted recovery instead of
   automatic text in the new app. Copy or explicitly Paste to recover it.
3. Open Microphone → Input Level, speak and switch inputs. Start dictation, then
   stop and close the window. Preview must yield/resume/release without saving audio.
4. Optionally select v3 and speak Swedish, or 110M for English. With Nemotron,
   pause, speak words to exclude, resume and finish. Restore your preferred model.
5. During drawing, use Draw on Screen → Undo Last Drawing; check the contact sheet.
   For private content, turn Include Page Details in Pins off before recording.
6. Enable Pause Supported Media While Recording. A supported playing control should
   pause; already paused media must stay paused. Resume manually. Try a local media
   file or typed URL through its new menu action.
7. Run `/Applications/Koett.app/Contents/MacOS/koett --control status`. It should
   return JSON with phase and selected model, without transcript text or secrets.
