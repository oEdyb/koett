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


## Source correction after this failed run

The pill now exposes virtual UIA Button/Invoke fragments through its own
DirectComposition HWND. The root owns virtual focus; Invoke posts an action
without activating a native child HWND. The original recording-target Paste
guard remains unchanged. Native Settings/History controls are unchanged.
Windows-target all-target metadata Clippy passes, including the new ABI and
state tests' compilation. No locally linked executable is claimed. The new
Rust provider must pass a rebuilt-artifact regression before PR readiness.
The isolated managed probe passes; it is separately labeled diagnostic only.

The source also enables NIF_SHOWTIP for NOTIFYICON_VERSION_4 and uses a stable
Koett icon identity with a current status tooltip, including after Explorer
restart. The guest diagnostic showed that omitting NIF_SHOWTIP suppresses
current tip text in Shell accessibility; adding it exposes the updated text.
That source correction also awaits its rebuilt-artifact runtime check.
Supply-chain outputs were regenerated for the explicit pinned windows-core
macro dependency; policy remains unchanged.


## Completed retest and remaining gate

The verified typing editor retained HWND 262172 / PID 9036 for every typed
character and fixture phase, and contains the typed phrase. A prior trial
that did not activate that editor is excluded from the pass list.
Pill geometry and compact native Settings both pass 100/150/200% scaling;
the Settings screenshots were retaken with its HWND actually foreground.
Narrator PID 2052 navigates the dark classic checkbox, Save and shortcut
button with correct roles/names. A small guest-output audio excerpt and its
approximate local ASR transcript accompany the keyboard JSON.

The strengthened UIA regression reports `passed: false` on 7a0e827 and the
host client exits 1. It records the tested executable hash. Future Paste
checks clear only the isolated test editor first and require exactly one
fixture insertion, preventing an earlier transcript from masking failure.
The changed-target case also compares both editors before/after Invoke.

Explorer restarted from PID 4676 to 3160 with Koett PID 8896 unchanged and
the stable tray GUID registered. Current tooltip text remained suppressed
without NIF_SHOWTIP; the separate native Shell diagnostic exposed updated
text when that flag was set. The source correction remains unverified in
an unmodified executable.

The new provider includes polite status names, explicit nullable COM output
initialization, retained-provider invalidation on window destruction, and
an independent registry for reentrant WM_GETOBJECT. A revision counter
prevents same-clock-tick phase changes from leaving stale accessibility or
reduced-motion state. Six pure state tests pass; ABI/provider tests compile
under all-target Windows Clippy but still need execution in Windows CI.
Native Linux tests (389 passed, 17 ignored), strict Clippy and release build
pass. Supply-chain outputs are current; policy is unchanged.

The guest completed a full Windows shutdown and is off. The transfer server
is stopped. M1 remains incomplete until the new linked executable passes
UIA Invoke, exact recovery Paste, switched-target refusal, pointer/state
regressions, virtual-action Narrator/status checks and tray-tooltip refresh.
