# Guest desktop checks

Run only against the isolated lab account and `Koett.exe --desktop-lab`.
The scripts require the serial bridge and initialize standard native UIA
providers. They record screenshots and foreground HWND/PID, using guest
mouse input only. They do not verify ASR, real media processing, or ink.

From the repository root:

```sh
python3 CrossPlatform/scripts/windows-lab/guest.py \
  CrossPlatform/scripts/windows-lab/checks/pointer-actions.ps1 \
  "$HOME/.cache/koett-windows/pointer-results.json" \
  CrossPlatform/scripts/windows-lab/checks/helpers.ps1
```

The archived e63e65a receipts used earlier check bodies. Current focus checks
activate only the guest test editor through guest mouse input and verify its
foreground HWND before continuing. They record the installed executable hash;
use a new output file for each artifact. The client returns failure for a
PowerShell error or an explicit `passed: false` result. `automation-actions.ps1` is a regression reproducer: e63e65a activates
the pill and refuses Paste. The fixed executable must preserve the editor
and paste the fixture. Then run `changed-target.ps1`: switching to a second editor must refuse Paste.

`states` opens an isolated WinForms typing editor. `pointer-actions` checks
real pointer controls and recovery. `scaling` needs Windows Display Settings
already open. `narrator` needs Narrator enabled and Koett running. `reduced-motion`
restores the prior animation setting. `theme-and-export` expects the lab
Unicode export fixture in Documents. `startup-off` changes only lab startup.
All fixture state IDs are documented in docs/windows-desktop-lab.md.

Read the result JSON and screenshots. A successful script exit alone is not
a passing check. History and export use the guest-only fixture; never point
these checks at an installed personal profile.


M2 real dictation checks use the concatenated helpers.ps1 and
 dictation-helpers.ps1 as the prefix, and launch-wav-dictation.ps1 switches a
running artifact from desktop-lab to real CPU WAV input. Copy fixtures/m2-editor.html
to the private HTTP transfer root and open it in guest Edge before browser-m2.
The terminal check reads text as data and never executes recognized commands.
Run on an isolated, freshly booted guest; checks retain lab windows for later
steps. Sequence: Notepad, controls, browser, terminal, focus recovery, history
failure, elevated start/test/probe, close elevated editor, combined failure.
Elevated start requests guest UAC consent; do not provide host input or add a
privileged Koett helper. Always restore readonly attributes and shut down.
The 1fa2169 artifact intentionally fails elevated-test and combined-failure;
the rebuilt fixes must pass those gates. See the current M2 receipt for scope.
