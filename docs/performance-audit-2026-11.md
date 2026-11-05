# November 2026 workflow performance audit

**Release gate: NO-GO for a renewed speed claim. Comparison with the video is
undetermined, not demonstrated faster, equal, or slower.** This is a partial
executed audit. The missing end-to-end comparison remains required before
calling the full audit complete. No release, installed-app change, or production
performance fix was made.

## Baseline recovered

Vault source: `Agent/02 - Content/YouTube/Completed/2026-08-13 Koett vs Wispr Flow/2026-08-13 YouTube Koett vs Wispr Flow.md`.
Recording card: same directory, `2026-08-13 YouTube Koett vs Wispr Flow Recording Card.md`.
The upload note records publication scheduled for **2026-08-18 18:30 CEST**,
YouTube `https://youtu.be/0xVRE7eX8Ns`, title *Can a Free Local App Replace Wispr Flow?*,
and the selected thumbnail claim **“4× FASTER”**.

The published-description text saved there says:

> In my controlled five-run test on my M5 Mac, Koett’s median release-to-paste time was 118.5 ms. Wispr Flow’s was 481 ms, so Koett was about 4.1× faster in this test. Both got every word right.

The recording card specifies the same Mac, microphone, sentence, and five runs.
The repository's [historical receipt](verification-receipts.md) narrows the
endpoint: Koett measured **paste-post**, Wispr its **finished-processing state**.
Neither is a timestamp proving text insertion in the target application. The
old 4.1× ratio therefore does not establish a symmetric target-receipt comparison.

The exact filmed executable/commit was not recovered. Use **v0.1.1,
`c72f83415b1d760c30316f07fc776f09b0ebdc0b`**, only as an explicitly labeled
fallback. Its local git commit date is August 26, after the scheduled upload;
do not describe it as the proven filmed binary. No old-versus-new run happened
on Linux: v0.1.1 is the native Apple Silicon application.

## Isolation and source

Linux source frozen at `c0343aef15dbb517b4d201e221d70c9666b1e6ca` (PR #99).
This branch started at `49efd70` and was fast-forwarded before measurement.
The installed private candidate's `candidate.json` independently names this
same source, with app binary SHA-256
`d9d52e6797c73b63c87ab7b181f532c48891d8e43af154c53c08a8864f662d32`.
The benchmark builds the engine from that source; it does **not** run the
installed desktop bundle or certify its runtime integration.

Host: CachyOS Linux x86_64, Ryzen 5 9600X, 12 logical CPUs, approximately 60 GiB
RAM. Initial reconnaissance load average was **35.12 / 18.61 / 14.68**. Other
jobs remained active; these are contended-machine observations, not idle-system
performance promises. The raw receipt records load at benchmark start/end.

Models were copied into this worktree's ignored `CrossPlatform/target/` tree.
No installed settings, transcript content, clipboard, microphone, pointer or
focus was changed. No transcript text is emitted or retained by the runner.
The inference subprocesses run in a network namespace with only loopback.
Build/run use `nice -n 10` and a `MemoryMax=12G` scope; build jobs are limited to
four. Build output is in the worktree, never `/tmp`.

Read-only disk accounting: the current-PC installed-style bundle contains
**275,473,457 logical file bytes**, including its **55,247,512-byte** app binary.
The copied v2 model tree contains **662,073,040 logical file bytes**, including
the test sample and generated bias vocabulary. This is file content size, not
unique allocated disk blocks (the model copy can share reflink blocks). Other
optional models are excluded. No same-definition old Mac bundle measurement
was obtained.

## Measurement coverage

| Requested measurement | Current evidence | Missing acceptance |
|---|---|---|
| Same-Mac old/current 5 s, 30 s, 2 min, 10 min stop-to-insert | Historical 118.5 ms paste-post only | All paired target-consumption timings, medians/worst cases |
| Shortcut press to actual capture-ready | Not measured | Real shortcut and microphone opening timestamps |
| Launch to ready; app idle/recording CPU and RAM | Not measured | App process measurements in an exclusive lab |
| Model load, empty/seed dictionary ASR | Runner and raw receipt below | App capture, paste, storage and UI overhead |
| 100 rapid dictations / leak soak | 100 sequential offline inferences only | Live sessions, lifecycle transitions and settled app RAM |
| Offline | Network-disabled local inference | Full desktop flow including delivery/recovery |
| CPU load / low memory | Naturally contended host; 12 GiB cap | Controlled idle/load pairs and actual low-memory recovery |
| Drawing/pins on/off; overlay startup | Not measured here | Paired real app runs, including capture readiness |
| Image-to-text gap | Historical Mac release-audit measurements below | Fresh current-build target-consumption measurements |

The sole authorized Mac child was interrupted: the Mac release auditor was
running a VM, so performance measurements were deferred. The PC also had an
active `koett-clean-kde` release-test lab; this audit did not open a competing
lab or borrow its input/clipboard. Neither interruption is a passing gate.

The release audit is absent from this main snapshot. Its original was read via
`git show 25ee1c3:docs/release-v0.3.0-audit.md`; the Linux version via
`origin/audit/linux-first-install`. The video checklist was read from main.
First-install gates are intentionally left to those owners.

## Existing image delivery evidence (not rerun)

Mac release audit `25ee1c3` records image-intake-to-text-insertion of **199.8 ms**
for a small image and **177.4 ms** for a 3.70 MB PNG. Its no-new-read-callback
fallback took **1,668.4–1,672.8 ms**, three runs. That identifies a slow path,
not a measured regression against the filmed version, which lacked drawing.
Linux PR #99 already changes sketch paste latency; this audit did not duplicate
or re-prove that fix. Do not combine Mac and Linux values into one claim.

## Fresh Linux results — 2026-11-05 UTC

[Raw metrics and fixture/binary hashes](../Benchmarks/Performance/2026-11/linux-asr.json).
Two CPU inference threads, no GPU inference. Empty and seeded conditions used
the identical WAV bytes and copied Parakeet v2. Three runs per cell. Values
below are **internal ASR milliseconds**, excluding file read, capture, delivery,
storage, dictionary spelling replacement and UI. They are not stop-to-saved or
stop-to-pasted measurements.

| Audio | Empty median / worst (ms) | Seed median / worst (ms) | Seed median penalty |
|---|---:|---:|---:|
| 5 seconds | 150.8 / 153.6 | 298.4 / 305.4 | +147.6 ms; 1.98× |
| 30 seconds | 842.4 / 935.6 | 1,613.8 / 1,695.4 | +771.4 ms; 1.92× |
| 2 minutes | 3,659.8 / 4,476.9 | 6,780.1 / 7,118.6 | +3,120.3 ms; 1.85× |
| 10 minutes | 18,256.0 / 20,010.0 | 34,207.2 / 38,656.1 | +15,951.2 ms; 1.87× |

Model-load observations: **1,012.8 ms empty / 1,007.5 ms seeded**, one process
each. These are warm-filesystem observations, not repeated cold-launch medians.
Sampled peak process RSS: **1,245,596 KiB empty / 1,283,212 KiB seeded**;
the difference was 37,616 KiB. This run did not reproduce the historical near
2 GB dictionary peak. Comparing these peaks with old idle measurements would
mix endpoints and workloads.

The independent **100-inference** run completed all 100 without a process error.
Median ASR **146.25 ms**, worst **162.5 ms**. Median sampled RSS over the first
ten and last ten completions was identical: **1,100,732 KiB**. This finds no
RSS growth at those checkpoints for this workload; it does not rule out app,
capture, clipboard, drawing, allocator or longer-duration leaks. It performs no
transcript correctness assertions. The 10-minute probes likewise test decoding,
not ten minutes of uninterrupted real microphone capture.

Host load averaged **15.09 / 18.26 / 15.58** at start and **7.50 / 12.59 / 13.83**
at end (1/5/15-minute averages). Seed ran second while load declined; randomized
or alternating process pairs on an idle machine are required for a stronger
causal estimate. The observed 1.85–1.98× ASR cost is consistent with the old
dictionary slowdown, but cannot validate its 0.32/0.60-second app endpoints.
The current `hotwords.rs` selects modified beam search with four active paths,
and decodes both plain and biased streams; that is a concrete extra-work cause.
Whether the seed is worth this cost remains an accuracy/product decision:
this performance probe intentionally does not score recognition quality.

Verification: the wrapped `cargo build --release --locked --bin koett-engine`
returned 0 in **1m 38s**. The network-disabled runner returned 0 with 124
inference records. A separate receipt check recomputed all eight median/worst
summaries and checked all expected counts. WAV headers verify exact durations,
mono, 16-bit PCM. Python syntax and `git diff --check` pass. No production app
source changed; full Swift/Rust application suites and cross-platform release
gates were not rerun and are not implied by these checks.

## Release decisions and truthful camera claims

- Keep raw dictation, Parakeet v2 and drawing off as the comparison control.
- Do not change dictionary behavior based only on speed: adoption runs a plain
  and a hotword stream with four-path beam search. Removing one can change text.
  Correctness needs a held-out accuracy check before a latency optimization.
- The previous 0.32 s versus 0.60 s stop-to-saved observation is historical,
  six runs each at load about 2. Fresh ASR numbers cannot validate that endpoint.
- A truthful retrospective is “In my earlier five-run M5 test, Koett posted its
  paste after a median 118.5 milliseconds.” Do not renew “four times faster” as
  a current comparison or promise all users that latency.
- Before release: obtain an exclusive Mac slot, identify or label the fallback
  old build, replay identical audio into private destinations, timestamp actual
  insertion, and close every missing acceptance row above. No regression fix PR
  is justified without a reproduced cause and paired before/after measurements.

## Reproduction

`Benchmarks/performance_audit.py` repeats the copied model's official speech
sample to exact 5/30/120/600-second PCM WAVs. These are dense repeated fixtures,
not diverse human speech or live capture. Three interleaved duration runs use
one model per dictionary condition; empty runs precede seeded runs, so thermal
and load drift are confounders. A separate empty-dictionary process performs
100 sequential 5-second inferences. It records model load, internal ASR timing,
10 ms sampled RSS and hashes, while discarding stdout text. RSS near a completed
decode is neither settled idle memory nor proof that the app leaks or does not.

```sh
systemd-run --user --scope -p MemoryMax=12G nice -n 10 \
  bwrap --bind / / --dev /dev --proc /proc --unshare-net -- \
  python3 Benchmarks/performance_audit.py \
  --binary CrossPlatform/target/release/koett-engine \
  --model CrossPlatform/target/performance-audit-20261105/model \
  --dictionary docs/dictionary-seed.txt \
  --output CrossPlatform/target/performance-audit-20261105/run
```
