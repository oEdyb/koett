# Desktop controls: incomplete parity increment

Base: feature checkpoint `72560e9`. Source changes are uncommitted. No install,
release, host desktop interaction, live preference writes or provider calls.

Implemented: native linked Raw/S1-mini selection, compact named header controls,
separate model names and Selected/Use/Download buttons, left-aligned Ask header
with Copy/Close, three-way Settings merge, and capture-start missing-device
resolution from real CPAL enumeration. Preview selection remains strict.

## Verified

- Audio module: 8 tests pass, including replacement ranking and audio preservation.
- Settings module: 6 tests pass, including stale editor merge and failed-write preservation.
- Strict locked all-target clippy, format and diff whitespace pass.
- Locked release passes; Rust 1.92 check passes.
- Windows source check passes using installed 1.98.0 toolchain with
  DOCS_RS=1 LIBSQLITE3_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1.
  Default stable lacks that target; no toolchain was installed.
- Private lab `koett-desktop`: light History/Settings matrix passes, exact Unicode
  Copy/search, 500px list/detail, empty/no-match/database-error recovery,
  long inputs/replacements, accessible Save and navigation.
- Dark/Sans 14/reduced-motion matrix passes at normal and 560px narrow widths.
- High-contrast History/Settings matrix passes, including 500px narrow, errors,
  long values and exact Copy/search.
- Model visual harness passes native Copy/Export/search and model-row captures.
- Cached S1 enable and Raw restoration persist from the actual segmented controls.
  A native busy-state accessibility failure was found and fixed by setting both
  segments insensitive explicitly. Final native rerun passes enable/Raw persistence,
  actual Recording → Ready, disabled S1 while recording, and 500px popover.

Full fixture outputs: `/tmp/koett-desktop-before`,
`/tmp/koett-desktop-after-light`, `/tmp/koett-desktop-after-dark-fixed`,
`/tmp/koett-desktop-controls-light`, `/tmp/koett-desktop-formatting-verified`,
`/tmp/koett-desktop-highcontrast`.
Raw fixture: `/tmp/koett-desktop-native-controls.py`.

Selected before/after PNGs are beside this README. The before build was the
existing release binary at task start; its source provenance was not independently
established. Baseline narrow harness failed at 517px; fixed header fits 500px.
Earlier 14pt build failed at 564px; compact Settings control fits 560px.
Default-renderer narrow captures contain black clipping; Cairo rerun captures
avoid it. Do not describe screenshot appearance as approved by the user.

## Remaining

This is not the requested complete increment. Tray/native menu breadth, three
persistent configurable global shortcuts and portal/X11 acceptance, durable Last,
volatile recovery/quit protection, folder action parity, active-device unplug
handling, actual microphone fallback fault injection, Ask/media/Nemotron smoke,
complete visual redesign remain unfulfilled.
Ask header changes compiled but were not exercised in native Ask acceptance.
No full suite was run; final integration remains the parent's responsibility.
No external blocker explains these implementation gaps.
