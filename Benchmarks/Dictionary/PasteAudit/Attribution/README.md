# Native regression attribution harness

These are opt-in private test harnesses. Never run the collectors directly from
this source directory. Never install them, read user History, grant host access,
or type into an existing visible application. Coordinate an unattended exclusive
host slot; existing microphone/Accessibility/event permissions are required.
The source apps are hardened without an audio-input entitlement. Acquisition is
excluded; a silent input can still clock the injected public PCM.

## Exact executed code and phase diffs

- `HostDriver.swift` is the exact executed foreign receiver, including the added
  `captureReadyMilliseconds` hook. It preserves the original clipboard only in
  memory and verifies restoration. Request UUIDs are bookkeeping; source PID,
  ordered capture generation, app event/clipboard generation, fixture progress,
  payload/marker equality and tail/length checks establish freshness.
- `run-cache.py` and `run-production.py` are the exact historical 8- and 4-receipt
  collectors. `resources.py` supplies the exact `proc_pid_rusage` counters.
- `shared-host-runtime.py` is the exact shared runtime those collectors imported.
  They evaluate its helper prefix only. **Do not execute its original 24-capture
  main** to reproduce this follow-up.
- Apply `cache-source.patch` or `production-source.patch` after the original
  `../Host/current-source.patch` to the `85dde51` source archive, with shared
  `../Host/AuditFixture.swift`. All runtime Swift source bytes reconstruct exactly
  in the rebuilt cache and frozen production trees. `../Host/FixtureInput.c` and
  its linked object supply the same recorder input seam. Old uses `c72f834` and
  `../Host/old-source.patch`, as in the original experiment.
- `prepare.py`, `add-stop-stages.py`, `reconstruct-cache.py`,
  `prepare-production.py` and `setup-runtime.py` preserve the actual preparation
  transformations. Their original flat private `.build/performance` layout is
  assumed. The cache source was reconstructed from those recorded transformations
  after its historical executable was overwritten; no historical hash is invented.
- `run-targeted.py` preserves the rejected recorder-retention experiment. It was
  deliberately stopped with SIGTERM after seven valid receipts and before its
  last planned recording. The cleanup handler restored the clipboard.

The retained recorder's real input Stop and file close remain inside Stop in
that experiment. Final production has no retained-recorder override. Production
and all measured current variants keep VAD enabled. Source trace instrumentation,
metadata copies and resource sampling are disclosed in the patches; synthetic WAV
copies occur during cleanup after target delivery. No timing is assembled from
separately measured ASR and paste durations.

## Build-only reproduction

Use a fresh output directory inside `.build/performance`. Supply only a public
FluidAudio model cache and the same frozen marked fixture directory. The latter
must include `manifest.json` and all four SHA-verified WAVs; the originals remain
in the private audit outputs, not user recordings.

```sh
python3 Benchmarks/Dictionary/PasteAudit/Attribution/prepare-repro.py \
  --identity 'EXISTING MATCHING DEVELOPER ID' \
  --model-cache '/PATH/TO/PUBLIC/FluidAudio' \
  --fixtures '/PATH/TO/FROZEN/guarded-fixtures' \
  --output "$PWD/.build/performance/fresh-attribution"
```

This builds only. It refuses an existing output and does not install, launch or
grant permissions. The full combined fresh command was not run end-to-end in this
audit; its individual build/sign/launch steps and exact patch reconstruction
were exercised. The original host preparer now archives `Tests` as well, avoiding
SwiftPM overlapping-source discovery when its test target has no directory.

After a separately authorized exclusive slot, `run-attested.py` in the prepared
private `attribution/` directory executes only the short nine-receipt integrity
and resource plan: three old 5 s, three production 5 s, two cache-mode 5 s, one
production 120 s. It computes source/app/driver/collector bindings before launch
and checks the app executable again after termination. Post-run source integrity
was also verified in the audit. Its output stays in the private `attested/` folder.

The historical collectors expect `Current.app`: clone `CacheBound.app` to that
owned private path before `run-cache.py`, verify cleanup/process exit, then clone
`ProductionFrozen.app` there before `run-production.py`. Preserve the frozen app
copies and hashes. Never overwrite a running executable or an earlier receipt
file. This repeats the short historical cohorts, not the old 75-minute matrix.

The old Audio Queue tap remains guarded to macOS `25G83`; do not change its private
layout offset on another OS by guessing. It can alter buffering and adds timed
old-only tap teardown. Physical microphone/key/rendering equivalence is not claimed.

## Provenance limits and verification

Historical cache/baseline binary SHA was not recorded before overwrite. Their
source is reconstructed, their collectors and guarded payloads are retained, and
the historical manifest says `null` for that executable hash. The final production
binary was frozen and hashed after its original cohort, then reused unchanged in
the new pre-hashed recheck. Fresh cache hashes are **not** assigned retrospectively
to historical receipts. `historical-provenance.json` and `attested/provenance.json`
keep those scopes separate. The latter records every runtime source file, binary,
shared runtime, driver, resource helper and fixture object/source hash.

```sh
python3 Benchmarks/Dictionary/PasteAudit/Attribution/verify-targets.py \
  --manifest Benchmarks/Dictionary/performance-20261105/guarded-fixture-manifest.json \
  --provenance Benchmarks/Dictionary/performance-20261105/attribution/attested/provenance.json \
  Benchmarks/Dictionary/performance-20261105/attribution/cache-receipts.json \
  Benchmarks/Dictionary/performance-20261105/attribution/production-receipts.json \
  Benchmarks/Dictionary/performance-20261105/attribution/target-receipts.json \
  Benchmarks/Dictionary/performance-20261105/attribution/attested/receipts.json
```

All 28 continuation receipts pass. The original 24 are unchanged and use the
original independent `../Host/verify-host.py` verifier. These cohorts must not be
pooled into a new four-length end-to-end distribution.

## Resource definitions

`idleAfterFiveSecondWarmup` follows a full 5 s ASR/paste warm-up, then three idle
seconds. `recordingResources.before/after` bound only the sampled recording window,
**before measured Stop**. They exclude that measured ASR. The historical four-case
production cohort did not sample post-measured-ASR memory. The new
`postMeasuredTargetIdle` waits for source ready after target receipt, then samples
three idle seconds; it is a distinct post-ASR/delivery observation.

Capture-ready means control request to the first nonzero injected-frame counter
observed via progress polling. Old observes an Audio Queue tap; current observes
an AUHAL callback. This is not physical microphone acquisition, a permission test,
or an assertion of equal old/current hardware buffering. Launch-ready uses the
source's Ready log with cached models and existing grants. Old model loading stays
combined with launch/setup; current's added model load/warm trace is in its logs.
CPU is private process user+system time as a percentage of one core; it excludes
other processes and accelerator utilization. RSS/footprint are process counters,
not all shared/model/accelerator memory. Logical bytes are not APFS clone growth.
The counters include fixture/control instrumentation and are not a Mac soak.
