# Global shortcut integration follow-up

Private KDE portal acceptance now passes: unassigned IDs produce an unavailable
status, desktop key assignment appears live in Settings, actual dictation toggle
and hold press/release work, live rebinding disables the old key and activates the
new one, and the Ask/media shortcuts dispatch. Ask was deliberately disabled;
this proves dispatch to its error state, not a cloud answer. ConfigureShortcuts
was invoked against the active session; editor window presentation is unverified.
The fixture changes only the disposable KDE desktop configuration through its real
KGlobalAccel service. It does not bypass the product portal path. Original failed
assertions and raw test runs remain under /tmp.

The former activation failure came from real KDE entries whose keys were empty,
although their IDs were returned. The product now exposes trigger descriptions,
counts only nonempty assignments, and reports missing bindings on initial results
and subsequent changes. Settings shows the actual current keys and refreshes.
Static value labels retain their accessible text rather than a mnemonic relation
that replaced it with the field name.

Independent review found and fixed legacy default collisions, dropped hold releases
while editing, and false X11 registration success. Further capture review found
same-button restart ownership, cross-row cancellation, and a flag race. Capture
now has generation ownership and central cancellation, bounded grabbed-key
forwarding, one editing snapshot per event and weak widget references.
The separate X11 evidence directory records final native acceptance and binary IDs.

Focused parent checks: settings tests 8, portal assignment test 1, editing latch
test 1, controlled-worker pending guard tests 2; strict Clippy, locked release,
MSRV 1.92, Windows source checking, Ruff, py_compile, formatting and diff checks.
No full-suite claim is made for the final capture edits.

One earlier reused-session run crashed in GTK accessibility while waiting after a
hold release. coredumpctl identifies private fixture PID 2278484, SIGSEGV, with
libgtk-4.so.1 + 0x440c5f calling g_variant_new with a null object path. The later
frozen-binary run passed. Its cause remains unresolved and is a final integration
investigation, not explained away by the passing rerun. No user data was involved.

Limits: private KWin and private rootful Xwayland only, not physical Pause, GNOME,
or every portal denial/partial/session fault. Native pre-worker Quit is covered
by a controlled in-process guard test plus scheduling smoke, not a deterministic
native stall fixture. Full stateful menus, media compatibility and packaging remain
open. No installed application, live preferences, user transcripts, Mac, commit,
push or public release changed. Parent-owned lab was stopped.
