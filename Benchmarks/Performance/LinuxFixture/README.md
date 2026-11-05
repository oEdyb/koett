# Private Linux workflow fixture

These are the executed audit scripts, not an app installer or a host-desktop test.
They require a private `agent-lab` KWin session, nested niri, Electron 43,
PipeWire/PulseAudio tools, wl-clipboard, a grabbed audit keyboard and copied public
models. The lab utility is an external prerequisite. See the workflow report for
exact source/app hashes and the measured limits.

Copy the fixture into `CrossPlatform/target/performance-live` before using it.
The controller assumes that location. Supply the audited installed-style bundle
under `bundle/`, and the verified copied v2 model and public generated
`speech-{5,30,120,600}.wav` files under
`CrossPlatform/target/performance-audit-20261105/{model,run}`. The original
`Benchmarks/performance_audit.py` creates those WAVs from the official model sample.
No model, audio, private HOME or transcript database is included here.

The lab uses the unique name `koett-perf-v2`. Check it is not owned by another run
before starting. `lab.py` redirects all XDG paths and the helper tools to a private
HOME. Launch `outer.py` through `run.py` inside `bwrap --unshare-net`; the outer
script asserts the owned private runtime before launching niri. It forwards only
KDE screenshot/settings portal interfaces; it excludes the outer clipboard and
remote-keyboard interfaces. `controller.py` asserts the private runtime name,
installs into its private prefix and sets the app's absolute process priority to
10 before launch. The grabbed Shift helper is
`CrossPlatform/evidence/sketch-20261102/lab-grabbed-shift.py`; it never targets a
physical keyboard.

`niri.kdl` must bind F8/F9 to the **private installed prefix's** `koett start` and
`koett stop`. Send those keys through the outer private desktop helper; inner
`wtype` did not dispatch compositor bindings in this fixture. The resulting key
measurement includes the helper/bridge and compositor dispatch, so it is an
upper bound, not hardware key latency. Use the private KWin fullscreen script
before measuring to stabilize the nested output layout.

The controller waits for `capture_ready=true`, written after the real CPAL
stream starts and its collector exists. Its command loop is driven by `send.py`.
`visual-check.py`, `hotkey-e2e.py`, `dictionary-e2e.py` and
`memory-switch-test.py` perform the short preflight checks. `suite-final.py`
then runs three full real-time trials per length, three feature trials each,
100 five-second live dictations and ten rapid start/cancel cycles.
`observe.py` and `resources.py` record metadata throughout. Start these observers
before the first take.

The 768 MiB high-watermark branch in `stress-tail.py` is a **recorded failed
responsiveness test**, not a required pass. It restores limits in `finally`.
`memory-hard-limit.py` separately verifies a 2 GiB hard maximum with no swap and
no artificial high watermark. `cpu-after-suite.py` waits until the main suite
passes, then measures three baseline/three loaded takes and checks that all eight
CPU-worker priorities are exactly 10. Do not overlap these cases with file-ASR
benchmarks, native/Rust builds or another desktop lab. Builds use four jobs,
`nice -n 10`, and a 12 GiB scope; the report discloses inherited ancestor limits.
All build output and temporary paths belong in the worktree/cache.

## Acceptance of a paste

- Each dictation gets a unique synthetic `AUDIT` marker through a literal
  replacement; the audio stays identical.
- The actual target must append exactly one text event containing this dictation’s marker.
- Clipboard hash/freshness checks happen **after** target consumption.
- Image receipt is timestamped after PNG decoding and insertion.
- Plain results must remove the previous image offer.

Nonempty text or a matching clipboard alone is insufficient: an earlier mixed
portal fixture retained stale text and produced false fast timings. Those runs
were discarded. A ten-minute case uses the runtime's automatic-stop timestamp;
a manual stop after the automatic paste is invalid.

Keep transcript-store content out of exports. Counts may be checked against the
fixture's own database, but never read or export its text columns. Stop only the
owned client and lab when done; preserve its numeric receipts and failure cases.
