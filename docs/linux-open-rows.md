# Linux open-row queue

The feature-parity table remains authoritative. This is the work queue after
rows 1–4, with row 5 proposed separately in PR #65. A verification gap stays open
until its actual acceptance check runs. Nothing below is declared OS-impossible.

## Row 6: remaining implementation and acceptance

- **Last transcript:** CLI guarded delivery implemented; real niri/X11 target
  consumption and image recovery matrix still need acceptance.
- **Undo drawing:** match Mac's recording-scoped undo of pending/captured frames,
  beyond Ctrl+Z on the current canvas; prove ordinary editor undo stays untouched.
- **Original paste target / per-app chord outside niri:** add trustworthy desktop
  identity checks for Wayland desktops; preserve copy/manual paste if unavailable.
- **Permissions:** first visible launch now opens the existing window once;
  background startup stays quiet. Actual microphone/keyboard/screenshot grants
  and denied/revoked paths still need acceptance.
- **Silence and empty ASR:** trace and exercise live silence through finalization;
  no failure pill, stale result or saved blank transcript.
- **Long dictation:** 30-minute hardware capture; measure memory, progress,
  pause/cancel, dictionary and chunk handling. Short/synthetic ASR is insufficient.
- **Bare Shift:** verify lost releases/capital/chord gestures and early-click
  behavior in a private lab; retain explicit keyboard-access fallback.
- **Displays:** add/remove during a recording and mixed-DPI coordinates/capture;
  currently a newly added monitor waits for the next recording.
- **Pin providers:** verify browser URL/selection and password rejection on other
  desktops/providers; keep image-only context when inaccessible. Browser setup
  must remain explicit. Incognito detection is absent on Mac too; do not imply it.
- **Clipboard bundle:** real consumption matrix in agent editor, native rich
  text, plain editor and terminal; ownership/race/portal timing coverage.
- **Media:** recheck anonymous Vimeo extraction; report upstream authentication
  failures, retain local-file alternative, never import browser cookies silently.
- **Ask:** real installed-panel Markdown/math and spoken interruption; theme,
  contrast, scaling, live preference changes and keyboard/screen-reader gates.
- **Packaging:** clean-user install/update/offline/recovery acceptance with native
  runtime dependencies; source builds do not close this row.

Model loading, choices, hotwords and spelling ordering remain under row 5 until
its review/CI/merge. Nemotron spelling-only behavior also exists on Mac; any
future acoustic implementation needs its own measured design. Crash journals
and click/spoken-cue keyframes are absent on Mac and Linux: named product backlog,
not silently claimed parity. The latter still needs a concrete behavior contract;
a crash journal needs explicit retention/privacy behavior before implementation.

## Deferred until after row 6

- **Tiled niri pointer context:** obtain trustworthy screen geometry or an explicit
  browser integration. Stock IPC omits tiled origins; AT-SPI may return zero.
  Never guess a link/image from invalid coordinates.
- **GNOME drawing:** Shell-compatible overlay/input integration, recording-scoped
  gestures, pass-through, capture exclusion, cancellation and accessibility.
- **X11 drawing:** overlay/input backend and equivalent capture/gesture gates;
  current layer-shell implementation does not provide it.

These integrations and all acceptance gaps remain open in `feature-parity.md`.
