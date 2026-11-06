# Koett for Windows — unsigned preview

This x64 preview runs locally on Windows 11. It is not a signed public release.
Physical microphones, Bluetooth/USB devices, sleep/resume and native ARM64 still
need hardware acceptance. Chrome and Firefox are not verified; Edge was tested.

Extract the complete archive, then open `Koett.exe`. Dictation uses a downloaded,
validated CPU model and works offline once that model is cached. The default
shortcut is Ctrl+Shift+Space. Settings selects the microphone, shortcut, toggle or
hold, model and optional formatting. Tray History, Words, drawing, pins and media
remain native Windows surfaces. Ask is shelved.

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
files or modified payloads cause a refusal; they are not silently deleted.
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
