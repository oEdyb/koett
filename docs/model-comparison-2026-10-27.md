# Controlled Linux model comparison, 2026-10-27

The cached Parakeet 600M v2 candidate made fewer word errors than Koett's
110M baseline on this bounded public corpus. A separate installed Whisper CPU
variant tied v2 on the 13 clips it completed within the wall budget. Whisper
Vulkan remains unranked: it failed model loading twice under shared VRAM
pressure. Keep defaults unchanged. V2 is a credible CPU upgrade candidate;
these results do not establish better accuracy than Whisper CPU.

## Corpus and scoring

Selected LibriSpeech `clean/test` row offsets `0,160,...,2400` before inference:
16 utterances from 16 distinct speakers, 141.990 seconds, 407 reference words.
The [OpenSLR corpus](https://www.openslr.org/12) supplies aligned audiobook
reference text; retrieval used its
[Hugging Face mirror](https://huggingface.co/datasets/openslr/librispeech_asr)
Dataset Viewer rows and individual FLAC assets. Attribution: Vassil Panayotov,
Daniel Povey, Guoguo Chen, Sanjeev Khudanpur and LibriVox readers;
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).

FFmpeg decoded each FLAC to mono PCM16 WAV at 16 kHz once. Each engine received
the same hashed WAV bytes, with no capture, resampling between engines, playback,
TTS or ASR-generated reference. Added the two existing public sherpa WAVs
(8.428 seconds, **unscored**, lacking independent gold) and a 65.365-second
concatenation of the first six corpus clips. That concatenation is synthetic,
has speaker changes and no added silence, and is scored separately. Total:
19 distinct inputs, 215.783 seconds; dataset response bodies totaled 2,697,297
bytes. Source metadata added small transfers; no model download or installation.

Normalization: Unicode NFKC, casefold, curly apostrophes to ASCII; replace runs
outside ASCII letters/digits/apostrophe with spaces; split whitespace and trim
token-edge apostrophes. Internal apostrophes remain. No spelling, number or
contraction equivalences. Exact word Levenshtein alignment uses tie preference
match, substitution, deletion, insertion. Corpus WER is pooled `(S+D+I)/N`,
not mean utterance WER. Strict spelling therefore counts `honourable/honorable`
and `court yard/courtyard` differences; error lists preserve this distinction.

## Results

- **110M CPU:** 13 substitutions, 2 deletions, 0 insertions; **15/407 = 3.69%
  WER**; 8/16 utterances without word edits.
- **600M v2 CPU:** 4 substitutions, 2 deletions, 0 insertions; **6/407 = 1.47%
  WER**; 12/16 utterances without word edits.
- **Whisper large-v3-turbo / Voxtype Vulkan:** no completed transcription or
  accuracy score. Both attempts terminated with signal 11 after Vulkan reported
  `ErrorOutOfDeviceMemory` for a 551,900,160-byte allocation. Logs identify the
  NVIDIA GeForce RTX 4070 and GPU device 0. Before retry, `nvidia-smi` reported
  1,072 MiB free of 12,282 MiB. Other GPU workloads were left running.

V2 corrected several baseline spellings/recognitions, including `Concorde` to
reference `Concord`, `Dina` to `Dinah`, and `vue` to `vague`. It still joined
reference `a Martha` into `Amartha` in the isolated clip. This is a small,
deterministic clean-read-speech sample with many literary names; it cannot
establish personal dictation, noisy audio or Swedish quality.

The natural 24.850-second clip used baseline Silero VAD plus two decode windows;
the candidate cache has no Silero and used two bounded windows without VAD.
Scores there were 2/72 versus 1/72. Restricting the comparison to the other
15 clips, all under 20 seconds, retains the result: 110M **13/335 = 3.88%**,
v2 **5/335 = 1.49%**. Do not attribute long-path differences solely to weights.

On the separate synthetic concatenation, baseline scored 6/171 word edits
(3.51%), v2 3/171 (1.75%). Baseline used eight VAD segments/windows; v2 used
four bounded windows. Neither emitted insertion edits here; one concatenation
does not validate real continuous dictation or streaming boundaries.

## Punctuation and resource observations

LibriSpeech's reference here is normalized text without punctuation, so no
punctuation accuracy score is available. V2 ended 13/16 corpus outputs with
sentence punctuation; baseline ended 2/16. Both inserted commas. V2's synthetic
long output included the awkward `terror against. some mighty grasping`.
More punctuation is observable; correctness needs separately annotated evidence.

## Separate Whisper turbo CPU extension

Ran `/usr/lib/voxtype/voxtype-avx2` directly, without changing the installed
symlink, daemon or user configuration. Package ownership: `voxtype-bin 1.1.1-1`;
binary version: `voxtype 1.1.1`; SHA-256:
`cc72629f07c7d95dafddc3651026a891ee7633c0083502a1fbd36037cac2a998`.
Its help confirms the same `transcribe [OPTIONS] <FILE>` command. The reviewed
file-command contract above applies; each completed run logs `use gpu = 0`,
one device/backend, CPU model buffers and `no GPU found`. No NVIDIA selection
environment was passed, and no GPU inference was retried.

Used the same cached weights, settings, scored WAV bytes/hashes and scoring
functions in a new private `.tmp/model-comparison-20261027/whisper-cpu/`
workspace. Inference retained the same background slice, CPU/memory caps and
90-second per-clip ceiling. The worker stopped after **260.044 seconds**;
including the initial help-probe attempt and restart, the elapsed interval was
under five minutes. The initial probe asserted the wrong executable name in
help; correcting that check required no recognition/configuration change.

Completed the first **13/16 natural corpus clips**, including the 24.850-second
natural recording. The remaining total wall budget expired about 14 seconds
into clip `4970-29095-0026`; its partial process output is unscored. Clips
`61-70968-0061` and `237-134493-0012` were not attempted. No synthetic or
unscored fixture was repeated. Recomputed all engines on the **same completed
13 clips, 341 reference words**:

- **Whisper turbo CPU:** S=5, D=1, I=0; **6/341 = 1.76% WER**.
- **600M v2 CPU:** S=4, D=2, I=0; **6/341 = 1.76% WER**.
- **110M CPU:** S=12, D=2, I=0; **14/341 = 4.11% WER**.

This is a budget-limited matched subset, not Whisper's result on all 16 clips
or a successful GPU benchmark. Whisper's completed runs had median fresh-process
end-to-end **18.615 s**, internal reported load **0.21 s** and process peak RSS
**2,019.8 MiB**, under the existing contention/caps. Its model-load timer covers
context construction; later state creation and transcription fall within
end-to-end time. Do not compare that internal timer to Koett as equal work.
Whisper ended 5/13 outputs with sentence punctuation; normalized references
still provide no punctuation accuracy ground truth.

Separate [CPU evidence](../CrossPlatform/evidence/model-comparison-20261027/whisper-cpu.json)
records the exact variant, version/help, outputs, scores, failed final process,
timings, log hashes, matching sample hashes and private settings. Validation
recomputed scores/counts, checked CPU-only logs, and verified the original
GPU `comparison.json` remains byte-for-byte unchanged. Both owned CPU services
have exited; no desktop/audio/clipboard operation occurred. This extension
changed only this report, its separate evidence and ignored research artifacts.

## Timing definitions and original Parakeet observations

Across the 16 corpus files, each in a fresh process:

- 110M median internal model load: **0.437 s**; median process end-to-end:
  **0.528 s**; median process peak RSS: **281.2 MiB**.
- V2 median internal model load: **0.852 s**; median process end-to-end:
  **1.148 s**; median process peak RSS: **1,060.2 MiB**.

End-to-end measures monotonic time immediately before process creation through
`wait4`, including file reading, model initialization, transcription and exit,
with up to 10 ms polling overhead. RSS is Linux `wait4.ru_maxrss` for that
process, excluding GPU VRAM and the Python runner. Koett's internal load includes
recognizer construction and optional VAD; short `transcribe_ms` excludes WAV
reading and waveform acceptance. No successful Whisper load/RSS comparison exists.
Fresh process does not mean disk-cold: integrity hashing warmed page cache.

All preparation/inference services ran in background under `agents-lab.slice`,
nice 15, shared two-core CPU quota and 6 GiB memory limit; inference services
also had a 5 GiB limit and 90-second per-process timeout. Shared GPU has no quota.
These observations describe this capped, contended run, not unrestricted speed.

## Identity, isolation and reproduction

Rehashed every baseline manifest file, every candidate receipt file and native
library. Candidate receipt:
`~/.cache/koett/model-evaluations/v2-trial-receipt.json`. The tested Koett CLI
SHA-256 was `0c008cc8443da3e53804cbd5ab428dca2f3553ecd03fce429007d9f474feb52f`,
unchanged after evaluation. Both Parakeet loaders used CPU and two threads.

Installed `/usr/bin/voxtype` resolves to `/usr/lib/voxtype/voxtype-vulkan`, reports
1.1.1, and accepts `transcribe FILE`. The existing 1,624,555,275-byte Whisper
model matches upstream SHA-256
`1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69`
at [whisper.cpp revision 5359861](https://huggingface.co/ggerganov/whisper.cpp/blob/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-large-v3-turbo.bin).

Reviewed [Voxtype release source](https://github.com/peteonrails/voxtype/tree/014c56105b2f071bb3457c51a95c38be380e5124):
the [file command](https://github.com/peteonrails/voxtype/blob/014c56105b2f071bb3457c51a95c38be380e5124/src/app/transcribe_file.rs)
reads WAV, creates a transcriber and prints processed text, without calling the
daemon/output delivery path. Local model resolution accepts an absolute existing
path and errors when absent. Source review does not prove installed binary
build provenance; the evidence records its local binary hash.

Private config copied the installed `[whisper]` values, changing only model name
to its absolute cached path; English and NVIDIA selection were preserved. No
thread override was added to Whisper (upstream default at most four). Each child
received private HOME/XDG config/data/state/cache/runtime directories, no desktop,
session bus or audio environment, and only AF_UNIX networking. Private state/data/
runtime remained empty; NVIDIA wrote shader cache files under the private cache.
No daemon, socket, clipboard or user-output path was invoked. All owned service
processes exited; no lab desktop was started. No product/settings/default changes
or commits were made by this research task.

Durable [machine-readable evidence](../CrossPlatform/evidence/model-comparison-20261027/comparison.json)
contains references, sample/model/binary hashes, exact alignments, all 38 successful
CPU outputs/timings, two Whisper failures and raw-log hashes. Ignored
`.tmp/model-comparison-20261027/` retains WAV/FLAC files, row metadata, raw logs,
reviewed source, `prepare.py`, `run.py`, `score.py` and `package.py`.

`prepare.py` enforces at most 20 inputs, 300 seconds and 100 MB, with bounded
requests; `run.py` verifies identity and resumes completed model/file pairs.
For a fresh rerun, copy the scripts into a new sibling ignored directory and run
preparation there, then launch `run.py` through `systemd-run --user` with the
slice/caps above and `RestrictAddressFamilies=AF_UNIX`; `score.py` recomputes
accounting. Passing `110m-cpu 600m-v2-cpu` selects CPU engines only; passing
`whisper-turbo-vulkan` selects Whisper. Preserve this evidence rather than
overwriting it. Scoring smoke cases and all log hashes/aggregate counts passed;
`git diff --check` passed. Product suites were not run for this research-only task.
