# M5 offline CLI groundwork — local c2a8b99

Private Windows executable SHA-256:
`c2a8b997d95b4e3fb94b7fd2b815da90cca538c6efa800dbab40d5d4cdd1ed26`.
Source base: merged M4 `5ed6e35`, with the CLI/helper source hashes in receipt.json.
This is not a signed distribution or completion of M5.

With the virtual NIC disconnected through QMP, the native console engine:

- Transcribed the cached compact model's 7.435-second WAV to JSON, exit 0.
  No API keys were supplied. A Unicode output filename succeeded.
- Rejected a missing offline model, input/output alias and malformed arguments,
  each exit 1. Missing-model failure created neither a model directory nor output.
- Kept foreground HWND 131208 / Explorer PID 4292 for each command. Input WAV,
  settings, History and Last Transcript hashes were unchanged. The latter three
  match M4's restored originals.

The final check requires all three app-data hashes. An earlier harness run had
an empty hash map due to PowerShell expression grouping; it is not used as proof.
The corrected run and transcript are in receipt.json. No benchmark was run.

Host impacted tests: three CLI parser cases and four local-build helper cases
passed. The new helper initially passed literal SDK include quotes to clang-cl;
`CC_SHELL_ESCAPED_FLAGS=1` fixes cc-rs tokenization. The actual SQLite/native
Windows engine then linked successfully through that helper.

The guest NIC link was restored through QMP and shutdown requested; the host
controller subsequently confirmed it off. Adapter status was still propagating
at shutdown, so only the successful QMP link-up response establishes restoration.
No host input or installed app was used. Chrome and physical Windows hardware
remain untested. URL/file conversion, native media UI, warm-engine media routing,
pause ownership and remaining automation gates are still M5 work. Ask remains
shelved on main; these checks do not restore it or imply browser-runtime support.
