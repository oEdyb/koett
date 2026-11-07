# Windows media and commands

Windows remains an unsigned x64 source preview. These features do not establish
physical-device, ARM64 or three-platform release parity.

## Media window

Choose **Transcribe media…** from the tray, or run `Koett.exe media`. Enter an
HTTP(S) URL or choose a local media file with Browse. Enter starts transcription;
Escape cancels active work or closes the idle window. The result stays in the
window after a failure or cancellation. Copy copies that media result, while
tray **Copy Last Transcript** continues to mean dictation.

The configurable media shortcut defaults to **Ctrl+Shift+T**. With a supported
public Edge document focused, it reads that document's verified URL and starts
without opening a window or changing keyboard focus. It never reads browser
cookies. InPrivate/password or unavailable/mismatched accessibility context is
omitted; the manual window remains available. Chrome and Firefox are untested.

Completed media is saved in the Media Transcripts folder. Options opens that
folder or transcript recovery and changes the shortcut. A recovery-folder or
History fallback stays visible. If every save fails, the raw result stays in
memory, the pill stays visible, and another media job or Quit is refused until
Copy or explicit Discard. Copy never replaces the clipboard with an empty result.

The preview requires `tools/ffmpeg.exe` and `tools/ffprobe.exe` beside the
executables; URL conversion also requires `tools/yt-dlp.exe`. Missing tools produce
a visible error and leave dictation usable. They are not bundled or silently
installed. The guest verification uses checksum-pinned private tool copies; a
release still needs a distribution and notices decision for those tools.

Conversion uses the same warm CPU worker as the desktop engine. Media metadata
names the actual Parakeet backend, including when dictation uses Nemotron.
Sources are limited to one item, two GiB and three hours, with known finite audio
duration required before conversion. Live streams, credentials in URLs, browser
cookies and unsupported sources are refused. Child processes run without a console
in a kill-on-close Job; cancellation, deadlines and output budgets reap them.
Temporary conversion audio is removed after processing. The dictation failed-ASR
audio contract remains separate.

## Pause playback

**Pause media while dictating** in Settings defaults off. When enabled, Koett uses
Windows Global System Media Transport Controls, not a Play/Pause key toggle.
It pauses only a playing session with Pause support, and resumes that same session
once when capture stops, fails or is cancelled. An already paused player is never
claimed. Playback, seek, track or current-session changes revoke the claim.
Unsupported players cannot be controlled; API failures are notices and never
prevent dictation. The caller waits at most 550 ms for a private worker.

A player which changes state without reporting it can defeat ownership detection.
Windows provides no atomic compare-and-Play operation; intervention concurrent
with the last state check remains a platform limit.

API references: [session manager](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager),
[playback changes](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssession.playbackinfochanged),
[timeline changes](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssession.timelinepropertieschanged).

## Console conversion

```powershell
.\koett-engine.exe media --help
.\koett-engine.exe media --model compact "https://example.com/video"
.\koett-engine.exe media --offline --model compact "C:\media\voice.mp3"
```

`compact`, `v2` and `v3` are explicit CPU choices. Offline conversion requires an
absolute local file and a validated cached model; it never fetches a source or
model. Online mode may download the selected model. The command prints one JSON
object with `ok`, `source`, `title`, `model`, `text`, `outputs` and `warnings`.
It does not write desktop History, Last Transcript, settings or the clipboard.
Progress goes to stderr. Ctrl+C terminates the console process and its owned
children. Abrupt termination can leave temporary conversion files; normal finish,
errors and supervised cancellation clean them up.

The lower-level `koett-transcribe-engine.exe --offline --model-version compact
--results-json OUTPUT WAV` remains available for 16 kHz mono WAV adapters.

## Running-app control

```powershell
.\koett-engine.exe app help --json
.\koett-engine.exe app status
.\koett-engine.exe app start
.\koett-engine.exe app stop
.\koett-engine.exe app cancel
.\koett-engine.exe app copy-last
.\koett-engine.exe app paste-last
.\koett-engine.exe app settings
.\koett-engine.exe app history
.\koett-engine.exe app dictionary
.\koett-engine.exe app media
.\koett-engine.exe app quit
```

`recovery`, `retry-save` and `copy-unsaved` expose native unsaved-text recovery.
`app status` also reports `unsaved_transcripts` and `recovery_busy`. Retry/copy
acknowledge a background operation; inspect status and the recovery window for
its result. Copy preserves a newer clipboard change and never discards pending
records. CLI Quit refuses pending text.

`toggle` is also available. Prefer explicit Start/Stop when retrying commands.
An acknowledgement means the app handled the request, not that recording, ASR,
paste or storage completed. Poll `app status` for `starting`, `ready`, `recording`,
`transcribing` or `error`, plus `paused`, `media_active` and `unsaved_media`.
Paste Last retains the original HWND/process and clipboard checks; it refuses a
new foreground target rather than pasting into another app.

Commands use a registered local Windows message, with no TCP listener. They must
run in the same interactive desktop and compatible integrity level as Koett.
Calls time out after 1.5 seconds. A timeout may follow an applied action: inspect
status before retrying. Native editing/import/export remains available through
standard UI Automation controls; these commands open the corresponding pages.

Success returns `ok: true`, `outputs: []` and `warnings: []`. Failures exit nonzero
and return `ok: false`, `code`, `error` and a concrete `fix`. Stable codes are
`invalid_arguments`, `unknown_action`, `not_running`, `command_unavailable`,
`acknowledgement_timeout`, `action_unavailable` and `media_failed`.

Ask remains shelved on main. This work adds no provider keys, cloud requests,
spoken replies or browser renderer dependency to dictation.
