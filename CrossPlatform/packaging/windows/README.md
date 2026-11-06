# Koett for Windows — unsigned preview

This x64 preview runs locally on Windows 11. It is not a signed public release.
Physical microphones, Bluetooth/USB devices, sleep/resume and native ARM64 still
need hardware acceptance. Chrome and Firefox are not verified; Edge was tested.

Extract the complete archive, then open `Koett.exe`. Dictation uses a downloaded,
validated CPU model and works offline once that model is cached. The default
shortcut is Ctrl+Shift+Space. Settings selects the microphone, shortcut, toggle or
hold, model and optional formatting. Tray History, Words, drawing, pins and media
remain native Windows surfaces. Ask is shelved.

## Setup installer

Open `Koett-Windows-x86_64-Setup.exe`, choose **Install**, then **Finish** to launch.
Setup installs for the current Windows account without administrator access and
adds **Koett** to Start and Settings → Apps → Installed apps.

Close Koett before installing a newer setup. Upgrade keeps settings, transcripts,
models and existing startup preference. Older installers refuse a downgrade.
There is no Windows automatic updater or scheduled update task. Start and new
taskbar pins use a stable launcher across upgrades. Recreate any pin made with
an earlier preview once, from the new Start entry.

To remove the app, choose **Koett → Uninstall** in Installed apps. Settings,
transcripts and models remain in application data. Added or changed version files, including files added to `tools/`, are moved to
`%LOCALAPPDATA%\Koett Preserved Files` and their location is shown. Incomplete
removal can be retried; a busy path is named with the next step.

These builds are unsigned: Windows has no verified publisher to show, and an
Internet download may trigger SmartScreen or policy blocking. Checksums do not
provide publisher trust. The isolated lab install does not establish reputation
for a future public download; no signing account has been provisioned.

### Moving from Koett Preview

In the old preview, turn **Start at login** off, then quit it before running the
native setup. This prevents its existing `Run\Koett` entry from launching the
old preview. After installing, enable startup in the new Koett if wanted. Both
installs share the same settings and transcript data; removing the old preview
through its own manager keeps that data. Setup does not delete a separate
preview install or take over an unrelated startup entry.

The ZIP remains a portable/advanced preview option:

## Optional per-user preview installation

From PowerShell in the extracted folder:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\manage-preview.ps1 -Action Install -Payload $PWD
```

Use this command only for a preview archive whose source and checksums you trust.
The process-scoped policy flag permits this unsigned preview script; it changes
no machine execution policy. No administrator access is needed.

The script checks every payload hash and architecture, installs below
`%LOCALAPPDATA%\Programs\Koett-Preview`, and creates **Koett Preview** in Start.
Close Koett before installing another version. Installation does not enable
startup. If startup already points into this preview installation, upgrade and
rollback preserve it with the selected version's path. Other startup entries
are left alone. The previous installed version remains available for rollback.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\manage-preview.ps1 -Action Status
powershell -NoProfile -ExecutionPolicy Bypass -File .\manage-preview.ps1 -Action Rollback
powershell -NoProfile -ExecutionPolicy Bypass -File .\manage-preview.ps1 -Action Uninstall
```

Uninstall removes verified preview files and its own shortcut/startup entry.
It preserves settings, models, history, dictionary and recovery data. Unexpected
files and modified payloads are kept in `Koett Preserved Files`; their location
is shown. User-provided `tools/` files carry forward on upgrade and setup rerun.
This script is preview management, not an MSI/MSIX installer or automatic updater.
Checksums detect corruption; they do not replace Authenticode signing or establish
SmartScreen reputation.

## Media and recovery

Transcribe media accepts a URL or local file. This archive does **not** include
FFmpeg, ffprobe, yt-dlp or the optional formatting runtime. Missing tools produce
an explicit error; ordinary dictation remains available. Tool distribution and
notices require a separate release decision.

Transcript recovery in the tray shows unsaved text, **Copy unsaved** and **Retry
save**. Copy does not delete pending records. Quit warns while text remains
unsaved; command-line Quit refuses it. Dictation options controls recording cues,
page details in pins and selected text. Page details off keeps pins image-only;
explicitly requested media URL reading remains independent.

`koett-engine.exe app help --json` lists running-app commands.
`koett-engine.exe media --help` describes standalone media conversion.
