# Native Linux Copy acceptance — passed, 2026-10-30

`acceptance.log` is a complete private Wayland run ending `PASS` and normal app
exit 0. `results.json` records exact bytes and candidate/runtime identity.
`history-before.png` and `native-copy.png` were visually inspected.

## Proven on the frozen candidate

- Native History pointer, AT-SPI button action and keyboard Space each replaced
  a unique fresh clipboard sentinel with the exact 2,831-byte Unicode fixture.
- Native search and actual list selection loaded the matching 88-byte Unicode
  detail; keyboard Copy returned those exact bytes. After no-results/clear,
  selecting the original row loaded its full 2,831-byte detail again.
- Modal Settings owned exactly one microphone preview stream. Native Save
  released it; returning to History and native pointer Copy returned all 88 bytes.
- Quit/restart preserved history; native History action copied the original
  2,831-byte fixture exactly.
- Real CPAL capture consumed the public model's English WAV through the private
  PipeWire monitor. One new v3 history row contained the 79-byte transcription
  in `v3-transcript.json`. Native selection loaded it, recovery Copy transcript,
  Last Transcript keyboard Space and native History Copy each replaced a fresh
  sentinel with those exact bytes. No CLI Copy substituted for any native proof.
- The final database had five synthetic fixtures plus exactly one v3 row. This
  count and the expected public-fixture word `country` were checked separately
  after the run; the reproduction now checks them inline as well.
- No successful WAV remained under the private app data directory. No capture
  stream remained; both private source-output and sink-input lists were empty.
  The app exited normally and `koett-release-copy` was stopped.

These native search/selection results exercise the final history-selection race
fix. They do not directly measure the short disabled-button transition while a
background detail query is pending.

## What actually failed in the earlier harness

The earlier black-frame diagnosis was incorrect. Inspected saved screenshots
showed Koett. Full-frame RGBA/default-segmentation OCR missed visible labels;
RGB/sparse OCR found the first Copy label, but sparse transcripts still defeated
full-frame segmentation. The helper now searches overlapping RGB screenshot
regions, requires one unambiguous high-confidence exact Copy label, and clicks
its visible location. It refuses missing or ambiguous targets.

`pointer-trace.txt` distinguishes the separate input failure. After resizing the
window, moving to the already-current compositor cursor location emitted no
pointer motion or new enter event. GTK received button events at its previous
surface coordinates and did not activate Copy. Forcing a private cursor exit and
re-entry produced fresh surface coordinates, button serial 231 and
`wl_data_device.set_selection(..., 231)`. This fixed the harness boundary; no
product clipboard defect was established. No added delay or lucky retry is used.
The corrected pointer path passed across multiple fresh app launches, including
before and after modal Settings in the complete run.

Other corrected fixture assumptions: forward Tab does not select every History
row; native list selection plus a full-detail wait does. Preview labels contain
bounded text with newlines flattened. Settings intentionally owns a preview
stream until Save. The transient recovery row naturally changes to Last
Transcript; the probe waits for the actual native action to appear.

Earlier failed logs were retained under
`~/.cache/koett/private-tests/release-copy-investigation-archive/`; larger private
app data, model copies, screenshots and full protocol traces stay in the cache.

## Identity and reproduction

Candidate binary SHA-256:
`b142eed9c4236344b2ddc60c421caa69d1a711d9aeca4a0f931013416085e0d6`.
This is a frozen source-built local preview candidate, not a public release or
clean-source attestation. `identity.json` records its path and relevant source
file hashes. `mapped-libraries.txt` records the actual process mappings. The
patched GTK 4.24.1 file hash and mapped path were asserted during the run:
`a6a8876857058a9b78ef681c9f57465a2cc664a8baa9a5123c7660598c51ef5a`.
This receipt does not establish native Wayland Copy on the GTK 4.6 floor.

From repository root, start only the isolated lab, then run:

```sh
agent-lab start koett-release-copy
agent-lab run koett-release-copy -- env COPY_V3=1 COPY_SKIP_REPRO=1 \
  LD_LIBRARY_PATH=/home/olle/.cache/koett/runtime-builds/gtk-4.24.1-21d3eb7/runtime/lib:/home/olle/.cache/koett/native/sherpa-onnx-1.13.5-linux-abi0-floor/lib \
  /usr/bin/python3 CrossPlatform/evidence/linux-release-native-copy-20261030/probe.py \
  --lab koett-release-copy \
  --binary /home/olle/.cache/koett/private-tests/release-copy-bin/koett-final-selection \
  --output /home/olle/.cache/koett/private-tests/release-copy-NEW-UNUSED-DIRECTORY \
  --theme dark
agent-lab stop koett-release-copy
```

Output must be new. Model files are manifest-checked and copied privately;
no writable host-model symlinks are stamped. The public WAV remains a read-only
input. Each Copy verifies its sentinel first and reads external `wl-paste` bytes
afterward; an AT-SPI true return or Copied label alone is insufficient.

## Scope and checks

The shared helper is integrated into `test_linux_parakeet_v3.py`, replacing its
agent `copy-last` shortcut with native History action and keyboard assertions.
That entire network-download/cancellation harness was not rerun in this focused
acceptance; this receipt covers the complete focused native journey above.
The X11 UI fixture's Ctrl+Alt+Space conflicted with Ask's default; only its
three chord/expected-label lines changed to Ctrl+Alt+D, preserving assertions.

Focused Ruff 0.17.2, py_compile and git diff --check passed. Production Rust
changes and full repository/floor gates are owned by the parent task. No install,
host input, host clipboard, user keys, physical audio, commit, push or release.
