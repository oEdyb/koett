# Green CI artifact 7a0e827 — outstanding-fix retest

[CI artifact](https://github.com/oEdyb/koett/actions/runs/41899257051/artifacts/13359390188)
from merge source `95f9d9834b969be9249a1d1a8c9a3352135fa4e2`.
Archive SHA-256 `1f49185ebbea51613bc6756766179b7c9e8740ceef1fb49bb4054793a68ce743`;
executable SHA-256 `77c59a0a4e077a56d8c887ee4085b94fe595131bb776ef58421809979f9ebf84`.
The guest hashes match the local archive/receipt. No executable patches.

Host had 44 GiB available before starting the 4-vCPU/8-GiB KVM guest. The
first boot stalled before the desktop and did not respond to ACPI shutdown.
With vCPUs paused, a local Btrfs reflink preserved the lab disk before one
controlled reset. The second boot reached the desktop. Read-only inspection
proved the first install request had not run; installation then completed.
No host pointer, keyboard, installed Koett or personal data was controlled.

Pass: visible 300×48 logical-pixel pill at 200%, correct gray/dim waveform,
readable dark checkbox text, native TogglePattern Off→On→Off in both themes,
compact 560×300 logical-pixel Settings, reuse of its HWND, reduced-motion
static transcribing and immediate Done appearance/dismissal.

**Failure remains:** standard UIA Invoke activates the pill despite the
previous restoration code. Pause/Copy move focus to Koett. Retry Paste
correctly refuses and leaves the original editor empty. The JSON captures
foreground HWND/PID at every step. PR remains draft; Milestone 1 is open.
Further diagnostics and source changes are not equivalent to retesting the
final unmodified CI executable.
