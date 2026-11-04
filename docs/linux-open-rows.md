# Linux open-row queue

The feature-parity table remains authoritative. This is the work queue after
rows 1–5, with row 5 merged in PR #65. A verification gap stays open
until its actual acceptance check runs. Nothing below is declared OS-impossible.

## Row 6: remaining implementation and acceptance

- **Last transcript:** CLI guarded delivery implemented; real niri/X11 target
  consumption and image recovery matrix still need acceptance.
- **Undo drawing:** recording-scoped implementation and private workflow are in
  PR #67; real capture, tray and ordinary-editor acceptance remain open.
  No global Ctrl+Z interception was added.
- **Original paste target / per-app chord outside niri:** KWin identity is now
  checked with per-window UUIDs and exact app-class rules. GNOME and other
  compositor adapters remain open; preserve copy/manual paste when unavailable.
  KWin portal grants/image consumption and the missing fallback clipboard
  protocol in the private KWin lab remain explicit acceptance/backend work.
- **Permissions:** first visible launch now opens the existing window once;
  background startup stays quiet. Actual microphone/keyboard/screenshot grants
  and denied/revoked paths still need acceptance.
- **Silence and empty ASR:** quiet completion is implemented and verified with
  the real 110M engine and isolated silent microphone capture. Silent drawings
  still compose; real physical microphone and other-model acceptance stay open.
- **Long dictation:** 30-minute hardware capture; measure memory, progress,
  pause/cancel, dictionary and chunk handling. Short/synthetic ASR is insufficient.
- **Bare Shift:** verify lost releases/capital/chord gestures and early-click
  behavior in a private lab; retain explicit keyboard-access fallback.
- **Displays:** live layer-shell drawing/pill refresh now handles monitor-list,
  geometry, connector and scale changes during a recording. Completed images and
  Undo survive; unfinished captures are invalidated with a pill notice. Private
  KWin virtual scale/remove/reconnect checks passed; physical mixed-DPI capture,
  portal screenshots and hotplug during a niri pointer pick remain open. See
  [the verification receipt](linux-monitor-hotplug-verification.md).
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

Row 5 merged in PR #65; its hardware accuracy/long-session acceptance remains open. Nemotron spelling-only behavior also exists on Mac; any
future acoustic implementation needs its own measured design. Crash journals
and click/spoken-cue keyframes are absent on Mac and Linux: named product backlog,
not silently claimed parity. The latter still needs a concrete behavior contract;
a crash journal needs explicit retention/privacy behavior before implementation.

## Implementation still open before the deferred integrations

Row 6 is not acceptance-only: Wayland identity/clipboard adapters beyond KWin
still need code. GNOME identity belongs with its deferred Shell integration.
Layer-shell monitor refresh is implemented; the physical/portal/niri-pick
hotplug cases above remain acceptance/integration work. The early-click gap
while the Shift guard arms still needs code. Existing copy/manual paste and
keyboard-access alternatives are fallbacks, not full parity. Early Shift clicks
come next, then the deferred integrations.


## Deferred until after row 6

- **Tiled niri pointer context:** obtain trustworthy screen geometry or an explicit
  browser integration. Stock IPC omits tiled origins; AT-SPI may return zero.
  Never guess a link/image from invalid coordinates.
- **GNOME drawing:** Shell-compatible overlay/input integration, recording-scoped
  gestures, pass-through, capture exclusion, cancellation and accessibility.
- **X11 drawing:** overlay/input backend and equivalent capture/gesture gates;
  current layer-shell implementation does not provide it.

These integrations and all acceptance gaps remain open in `feature-parity.md`.
