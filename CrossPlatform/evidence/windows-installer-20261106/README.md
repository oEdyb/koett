# Windows installer acceptance — 2026-11-06

Windows 11 Enterprise Evaluation build 26100 in the existing isolated KVM/QEMU
lab; 4 vCPU, 8 GiB, nice 19. Only guest input was controlled. The host had more
than 12 GiB available at each boot. No personal installation or data was used.

## Passed

- Native setup install, Start shortcut launch, Ready and native Settings.
- Native microphone preview on the virtual HDA device; held saved shortcut starts
  recording and Cancel returns Ready without changing the clipboard sentinel.
- Installed binary: offline CPU WAV dictation pastes once into Notepad with the
  same foreground HWND/PID. The fixture contains public-domain Phoebe narration.
- Previous 0.2.0 lab fixture install and actual older GUI launch, followed by
  native wizard upgrade to 0.3.0. This is a constructed previous-version fixture,
  not an assertion that a public 0.2.0 Windows setup existed.
- Config, History and Last Transcript hashes unchanged by upgrade. An enabled
  startup entry retargeted to the new version. Initially-off startup remained off
  on the initial install. A simultaneous setup and a downgrade both refused.
- Final setup artwork repair/upgrade and Start launch passed with the exact
  setup hash in `build-identity.json`.
- Windows Settings → Apps → Installed apps → Koett → Uninstall invoked the
  registered uninstaller. Native Uninstall completed. Root files, Start entry,
  startup entry and uninstall registration were removed. The three data hashes
  were unchanged; the original config hash and startup-off state were restored.
- Defender antivirus, service and real-time protection were enabled at the end.
  Nothing disabled or excluded them. The guest was shut down after testing.

JSON files retain foreground HWND/PID and assertions. Screenshots were extracted
without changing image content; final welcome, app registration, Settings and
uninstall screens were visually inspected. No binaries, model or audio files are
committed. The setup uses local Windows MSVC debug application bytes, not a CI
release artifact. The final combined manual CI separately builds release assets.
Background Windows Edge/WebView2 processes are recorded in the paste receipt;
this test checks the installed app's module independence, not their absence.

## Harness corrections and limits

A fast key tap initially tested the saved Hold shortcut incorrectly; the held
chord passed. A raw `Get-Content` result made PowerShell 5 expand file-provider
metadata during JSON serialization. Parsed values/hashes fixed the harness;
the bounded downgrade retry passed. A download-and-Invoke-Expression command was
blocked by Defender; saved plain scripts and the existing serial lab route were
used instead. Protections were left enabled. The registered uninstall window
has a trailing space in its NSIS title; discovery corrected the exact match.

Physical microphones, Bluetooth/USB devices, sleep/resume, native ARM64, Chrome,
Internet Mark-of-the-Web/SmartScreen reputation and certificate signing were not
retested. The virtual microphone cannot establish real input quality. No release
was published and no package-store submission was made.

Local compiler packages came from Debian's official stable pool:
`nsis_3.12-1_amd64.deb` and `nsis-common_3.12-1_all.deb` under
`https://deb.debian.org/debian/pool/main/n/nsis/`. Their SHA-256 values are recorded
in `build-identity.json`. The original notice comes from upstream NSIS tag v312.
