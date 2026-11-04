# Windows desktop lab

Run `Koett.exe --desktop-lab` in an isolated Windows user profile. This explicitly
skips the speech engine/model download and starts the real native desktop shell.
It is a visual/accessibility fixture, not evidence of microphone or ASR behavior.
Normal launches ignore the fixture message entirely.

Send `WM_APP + 0x52` (`0x8052`) to class `KoettOwnerWindow` with wParam:

| Value | Action |
| --- | --- |
| 1 | Start silent recording UI, preserve current foreground target |
| 2 | Transcribing UI with frozen strip |
| 3 | Done with fixture text |
| 4 | Persistent not-pasted fixture with Copy/Paste |
| 5 | Persistent rescue error |
| 6 | Five-second error |
| 7 | Hide pill |
| 8 | Open real native Settings |
| 9 | Open real native History |
| 10 | Toggle paused UI |
| 11 | Toggle drawing glyph fixture (no ink capture) |
| 12 | Media-result fixture, 10-second timeout |

Take a guest screenshot and foreground HWND/PID before and after every step.
Test ordinary startup and tray behavior with a normal launch separately.
The lab does not simulate speech recognition, delivery acknowledgments or save
success. Use actual recordings and fault injection in Milestone 2 for those gates.

Pill action controls are owner-drawn native Win32 BUTTONs, so standard Windows
accessibility providers expose their labels and Invoke behavior. Their runtime
Narrator behavior and focus preservation still require guest verification.

## Local guest

Host packages: `sudo pacman -S --needed qemu-desktop edk2-ovmf swtpm`.
The cache directory is private (0700). The local test account has no password;
there is no public VNC/RDP listener. VNC, QMP and the serial command bridge use
private Unix sockets. Do not expose those sockets or forward them publicly.

`CrossPlatform/scripts/windows-lab/` contains the unattended deployment seed,
limited-integrity interactive bridge and guest lifecycle scripts. The bridge
executes test scripts only inside the guest. Keep both disk and official ISO out
of git, under `~/.cache/koett-windows/`.

For the existing installed guest:

```sh
python3 CrossPlatform/scripts/windows-lab/guest_control.py start
python3 CrossPlatform/scripts/windows-lab/guest_control.py status
python3 CrossPlatform/scripts/windows-lab/guest.py test-script.ps1 result.json
python3 CrossPlatform/scripts/windows-lab/guest_control.py stop
```

Start requires at least 12 GiB MemAvailable, uses exactly 4 vCPU / 8 GiB and no
host display. It records guest audio to the cache instead of host speakers.
Stop requests a clean ACPI shutdown and refuses to force-kill on timeout.
Initial `--install` requires an official verified ISO and prepared seed; it
refuses an existing guest disk because the answer file wipes guest Disk 0.
Never use the installation answer file against a physical machine or other disk.

Guest evidence must distinguish the original CI artifact, a later CI artifact,
and fixture-driven UI states. Record the artifact receipt's source commit;
GitHub PR artifacts identify their tested merge commit, not only the branch head.
