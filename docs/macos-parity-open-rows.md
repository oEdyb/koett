# Mac parity: remaining scope after the 0.3.1 guest checks

Completed in separate PRs: draw lock #130, private Chrome pin metadata #131,
cold Chrome document URLs #132, and VoiceOver-safe lock shortcuts #133.
Movable pill #127 and configurable recording limit #125 were already merged.
The installed notarized v0.3.0 remains unchanged; these merges are source work,
not authorization for another install, ZIP, tag or publication.

## Remaining implementation or provider limits

- **Private mode outside English Chrome/Chromium:** other browsers, channels and
  translated mode markers still need reliable window-specific signals and
  fixtures. Explicit image-only pins are available now. Captured pixels remain
  visible regardless of metadata filtering; do not claim universal detection.
- **Renderer-specific pin fields:** document URL/title have a cold-tree fallback;
  link/image/selection/player data still require a usable provider web tree.
  Unavailable fields stay absent, not invented.
- **Nemotron silence trim:** audio already passed through the stream cannot be
  retroactively trimmed. A live VAD/segmentation path needs accuracy validation;
  quiet empty results and existing Parakeet failure recovery remain available.
- **Acoustic dictionary parity:** Mac uses cheap spelling by default. Its v2 CTC
  sidecar stays experimental/default-off after the measured overhead; v3/110M
  and Nemotron have no validated equivalent hook. Re-enabling extra inference
  without the missing accuracy/runtime proof would undo the performance fix.
- **Crash during active capture:** no bounded crash-safe journal exists. This
  needs a separate retention/privacy and repair design; completed-transcript
  recovery must not be described as process-kill recovery. The optional row
  remains open across platforms.
- **Media controls:** explicit accessible Pause is opt-in, player/provider
  dependent, and resumes manually. Unsupported controls are untouched. A blind
  media-key toggle could start playback and is not an acceptable fallback.
- **Vimeo:** anonymous extraction's known authentication failure is upstream;
  user-selected local files are the available alternative, without cookie import.
- **Click/spoken-cue keyframes:** no precise cue/acceptance contract is established
  on any platform. Explicit drawing and pins remain the shipped alternative.
- **Intel and distribution:** the public Mac contract is Apple Silicon. No Intel
  artifact, new 0.3.1 notarized release or automatic-update publication is claimed.
  Shelved Assistant work is excluded from this queue.

## Still untested on physical hardware

Physical Option/Shift/draw-lock chords, pen/tablet input, VoiceOver user navigation,
mixed-DPI/rotated/multiple displays and cable hotplug; real microphone/Bluetooth
handoff and long sessions; real 60-minute expiry and an 11-minute Nemotron run.
Other-player/native appearance/high-contrast/scaling combinations are not blanket
accepted. Existing guest receipts and targeted tests remain narrowly scoped.

[Real native T3 0.1.25 repeat-paste guest acceptance](macos-t3-repeat-paste-verification.md)
now supplements the existing Chrome/native receiver checks. It does not prove
universal target consumption or replace testing a real active conversation.
