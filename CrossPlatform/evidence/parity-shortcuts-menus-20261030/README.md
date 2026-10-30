# Shortcuts and compact menus — partial implementation

This increment is NOT complete and does not establish global shortcut support.
No installation, live configuration edits, commit, push or Mac source changes.

Changed within already dirty shared files: `src/settings.rs`, `src/linux.rs`,
`src/linux/settings_view.rs`, `src/linux/menu.rs`, `src/linux/window.rs`.
Added `scripts/test_linux_shortcuts_menus.py`. Other existing shared edits retained.

Implemented: independent serde-default Ask Ctrl+Alt+Space and media Ctrl+Shift+T
preferences alongside existing dictation; independent three-way editor merge;
semantic chord collision validation before save, including equivalent modifier
names/order; empty bindings allowed. X11 native capture controls, Clear and Reset;
Wayland desktop-managed labels. X11 per-action registration/latches and rebind
register-before-unregister retain the previous registration after failure. Portal
requests distinct dictation/ask/media IDs and dispatches press/release with repeat
latches. Media opens the existing manual URL workflow. LinuxEngine guards capture
commands from crossing between Ask and dictation. GTK groups existing operations
under Ask, Transcripts, Recovery and Preferences; adds Ask and Cancel Ask actions.

Primary API references (2026-10-29):
- https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html
- https://docs.rs/global-hotkey/0.8.0/global_hotkey/struct.GlobalHotKeyManager.html
- Installed ashpd 0.13.13/global-hotkey 0.8.0 source and lockfile. No upgrades.

Passed: focused shortcut/settings Rust tests, fmt, strict locked all-target Clippy,
locked release, Rust 1.92 source check, requested Windows 1.98 source check,
Ruff for added script, git diff --check. Logs alongside this receipt. Native binary
was the new release for focused-light-v2, focused-dark and queued-native runs.
The original native-light run started before the release completed and failed an
existing stale five-item tray assertion; it is NOT current binary proof.

Private lab only: `koett-shortcuts-menus`, isolated XDG paths, synthetic silent
PulseAudio/ALSA input, private accessibility/clipboard/session bus. Actual tray
DBus Settings activation passed independently of rendering. Native keyboard menu
opens; narrow submenu labels asserted. Light/dark screenshots inspected. Final
light journey sends Start followed by actual tray DBus Quit, observes Recording
and a surviving process, stops capture, verifies Ready, sends disabled Ask,
asserts Ask Error and ordinary Quit exits zero. This demonstrates native busy
protection under this scheduling; it does not deterministically pause the worker
before its first status or prove every queue interleaving. No real question, paid
provider, physical microphone or host transcript used.

OPEN / do not claim done:
- Deterministic pre-first-status queued Start/Quit acceptance and cancelled Ask.
- Actual private X11/portal activation, repeat/hold/rebind/denial fault tests.
- Portal RegisterHostApp failed: App info not found for com.olledyberg.Koett.
  Portal dispatch therefore was not exercised. This is lab app registration,
  not evidence that the compositor protocol cannot support these actions.
- Existing configure_shortcut creates a separate empty session; route editing to
  the live bound session. Portal partial registration/change handling and active
  hold cleanup require completion, including missing Ask/media ID reporting when
  all previously listed bindings cause BindShortcuts to be skipped.
- X11 failed rebind retains working in-memory binding but persisted desired value
  can still differ. Collision with another retained old binding needs accounting.
- Complete stateful microphone/mode/model/formatting/login/About menu and tray
  submenus, busy disabling, checkmarks, JSON command/help parity still pending.
- Exact Last/recovery retained-text acceptance, chord normalization on persistence,
  native settings save/restart/collision acceptance, pointer/checkmark coverage.
- Parent final full suite and independent review.

Selected receipts/screenshots only; audio, model/cache, DB and transcript fixture
bytes remain outside tracked candidates. Failed attempt logs retained, assertions
in existing harness untouched. All owned app/registry/watcher processes exited;
private audio modules unloaded by harness. Lab stopped and status confirmed.
