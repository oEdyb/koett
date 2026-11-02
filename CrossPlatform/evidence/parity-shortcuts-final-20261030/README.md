# Three global shortcuts follow-up — incomplete acceptance

Shared uncommitted work preserved. No commits, push, host installation, Mac,
skills, host input, user preferences, live microphone or paid/cloud calls.

Source changes: transactional X11 key-set registration (including swaps), rollback
of desired shortcut fields on registration failure while preserving other loaded
preferences, held-key suppression across successful rebind, semantic normalization
on persistence, and shortcut capture atomic cleanup on Escape, success, failure,
Clear and Reset. Existing window-close cleanup retained.

Portal: requests enabled IDs on every new session (listed shortcuts are previous
session history), reconciles listed plus returned IDs, ignores disabled IDs,
filters every signal by exact session, subscribes to session closure, and releases
both held actions on changes and closure. Editing routes through a command channel
to the live session, with explicit version-1 rejection and propagated errors.
Commands: shortcuts-help, shortcuts-status, shortcuts-edit. Settings shows backend
or registration failure. X11 status exposes desired and active state/failure.

Fresh checks: 12 focused shortcut tests and 2 controlled-worker queue tests passed;
strict locked all-target Clippy, release build, Rust 1.92 check, requested Windows
1.98 check, Ruff, fmt and diff checks passed. No full suite or delegate reviews.
The test-only EngineHandle constructor controls the real LinuxEngine command queue;
it asserts capture is pending before any worker status and cleanup permits exit.
It does not invoke the native Quit action at that deterministic pause.

Private native smoke: actual tray DBus Settings, native narrow menu, Start followed
by tray Quit survives Recording, disabled Ask followed by Quit exits successfully.
See native-results.json and queued-start-quit.png (inspected).

Private portal: installed com.olledyberg.Koett.desktop before app launch under the
lab XDG_DATA_HOME/applications. Real KDE RegisterHostApp succeeded; consent showed
all three defaults. Real portal returned all three IDs and accepted ConfigureShortcuts
against the existing session (portal-edit-result.json). The selected screenshot was
inspected: it shows Koett, not a configuration dialog. Only successful method
invocation is established, not visible editor presentation. Actual injected
Ctrl+Shift+Space did NOT produce Recording; the retained harness fails that assertion.
No claim of unsupported compositor follows from this observation.

Still required before completion: diagnose actual portal activation; focused mock
portal denial/partial/foreign-session/closed-session acceptance; actual private X11
activation/release/repeat/hold/swapped rebind and occupied-key failure; native Settings
save/reopen/collision/error-focus; exact Last/recovery across settings and restart;
deterministic native Quit before first worker status including cancelled preparation.
Xvfb is absent here; Xwayland exists but no private X11 fixture was completed.
The assigned subsystem is NOT complete, and the native harness is intentionally
red for missing actual activation. Inventory records these limits.

All owned harness/app/accessibility/watcher processes exited, private audio modules
unloaded; agent-lab koett-shortcuts-final stopped and status confirmed. Raw private
config, transcripts, database and audio remain temporary, outside this directory.

API sources read: pinned ashpd 0.13.13 Session/GlobalShortcuts and global-hotkey
0.8.0 API; official GlobalShortcuts v2 specification:
https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html
