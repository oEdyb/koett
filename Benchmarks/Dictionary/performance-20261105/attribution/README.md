# Mac regression attribution

Original 24 acceptance receipts in the parent directory remain unchanged.

`stage-profile.json` contains 60 same-frozen-WAV, warm native component/ablation
trials (three repetitions, five modes, four lengths), reproduced by
`DictationStageProfileTests`. Manual split and production outputs match; every
mode produced identical text per length. These are file-ASR component timings,
not reconstructed Stop-to-target times.

`cache-receipts.json` contains eight actual Stop-to-owned-target receipts from
an instrumented cache ablation: three 5-second before/after pairs and one
120-second pair. `production-receipts.json` contains three 5-second and one
120-second receipts from the final cache implementation, with instrumentation
and the previously disclosed recorder-buffer seam. Each group has a new source
process and one complete five-second warm-up. All receipt payloads match the
marker, fixture hash, source PID/generation and owned clipboard generation;
clipboard restoration passed. Output is identical within each tested length.

The menu probe measured about40 ms inside `SMAppService.mainApp.status` out of
about42 ms menu rebuild. The production cache removes that busy-state lookup.
Actual Stop-to-ASR-start fell from about69–88 ms to32–38 ms. End-to-end results
still vary with ASR: a final 5-second run took181 ms and the single final120-second
run638 ms. These data do not establish a universal end-to-end speedup or ceiling.

The rejected retained-recorder experiment stopped after seven verified receipts,
with its next planned capture canceled before it began. Audio-unit destruction
was about0.4 ms and moving it after paste did not explain the fixed cost.
Its collector ends with an intentional SIGTERM traceback after verified cleanup.

All WAVs and text are public synthetic fixtures. Captured WAVs remain private;
only format/size metadata is committed. Resource counters describe the private
process under the PCM seam (including fixture/control work), not an installed-app
soak or all accelerator memory. Disk counts are logical bytes, not APFS clone
physical growth. The final report contains the release recommendation and limits.
