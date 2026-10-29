# GTK application-root accessibility crash

Investigated 2026-10-29 CET. Evidence folder name follows assigned scope.

## Finding

The known private Koett core and an independent GTK fixture hit the same GTK defect: application-root `GetChildAtIndex` falls through to a hidden toplevel when no visible child matches, retrieves an unrealized accessibility context with a null path, and passes it to GVariant's object-path constructor. This is reproducible without Koett, audio, cached Atspi proxy objects, registry replacement, or overlapping product callbacks.

Four native fixture runs crashed (PIDs 2337841, 2339891, 2339968, 2341736); the keep-visible control survived and returned the same valid object path twice. The last failing run explicitly verified the registry bus owner's PID matched the fixture-owned daemon; it remained alive at the crash. Earlier runs only checked daemon liveness. No claim that any clean Koett run fixes this.

## Exact mapping

Installed GTK `1:4.24.1-1.1`, GLib `2.90.1-1.1`, KWin `6.8.1-2.1`; full package/build IDs and hashes in versions.txt.

* Known core PID 2278484: GTK return PC `+0x440c5f` is immediately after `g_variant_new` at `+0x440c59`, within local ELF symbol `handle_accessible_method.part.0` (starts +0x440820).
* Tagged GTK source `gtk/a11y/gtkatspiroot.c`, function `handle_accessible_method`, GetChildAtIndex branch, lines 276–306; serialization line 305.
* Disassembly obtains context from window, path via `gtk_at_spi_context_get_context_path`, and constructs `((so))`. Context-path getter +0x43db30 returns the field at +0x78.
* Known core frame 5: r13 is context 0x561c26240cd0; field +0x78 is zero. Invocation method memory contains GetChildAtIndex. r12 is the valid bus name `:1.9`. This is a null object path, not a null bus name.
* GIO +0x12e79a maps to `call_in_idle_cb`, immediately after dispatch through the exported-interface method function pointer. It is an incoming D-Bus method dispatch, not a Koett asynchronous completion callback.
* Independent fixture PID 2337841 has identical GTK +0x440c5f, GIO +0x12e79a and GLib variant-construction offsets. See native-core-info.txt.

Mapping uses installed ELF local symbols/disassembly and tagged source, not guessed function names or mismatched downloaded debug symbols. The rebuilt release executable cannot establish the exact source identity of the deleted crash executable; no Koett-frame inference is needed for this diagnosis.

## Mechanism and discriminating checks

The handler scans toplevels but assigns `window` before filtering visibility. If the requested visible index is absent, it retains the final hidden window instead of returning an absent-child reference. A never-presented window's accessibility context has no exported object path. A retired context also clears that path in `gtkatspicontext.c`, `gtk_at_spi_context_unrealize`, lines 1635–1653.

Fixture: initialize GTK, retain one never-presented main window, present a disposable Settings window; query root child zero successfully; destroy Settings; query root child zero again. Native Gtk through PyGObject only. Direct gdbus requests avoid client-side Atspi caching entirely. The second request crashes. Keep-visible control changes only whether Settings is destroyed and survives.

Koett creates a hidden main window when launched without visibility; Settings closes with normal GTK destruction. Its visible-window list may change while the accessibility client traverses child indices. The test's `walk` reads child count then children through separate D-Bus calls, allowing index retirement between calls. This explains a timing-sensitive trigger without requiring unsafe Rust or stale app-owned accessibility pointers. Existing product code was read only and may be changing concurrently.

The original script manually starts registryd, but registry retirement/replacement is unnecessary for the native reproduction. The private lab supervisor itself does not explicitly spawn registryd. The final run proves correct fixture ownership of registryd on the private accessibility bus. This does not reconstruct every historical owner transition in the original session.

## Smallest justified recommendation

Fix GTK's application-root child lookup to represent an unmatched visible index as the standard absent accessible reference, and never serialize an unexported context path. The upstream fix must cover both no-match and invalid/unrealized context handling using GTK's existing accessible-reference conventions, with a regression test for hidden toplevel plus retired visible child. This is a dependency defect; coordinate any tested dependency patch separately. No production workaround, accessibility disabling, sleeps, warning suppression or speculative Rust lifetime patch is justified by this evidence.

Remaining uncertainty: the precise original requested index and identity of the final hidden window are not decoded from the old core; its instruction path and null context field are confirmed. No full original shortcut/audio workflow was rerun. No patched GTK build was tested. No upstream issue/fix search was performed, so fix availability is unknown. Preview-widget retirement is not required by the fixture and was not separately instrumented.

## Re-run and scope

Start private `agent-lab koett-gtk-a11y-diagnosis`, then run this evidence folder's reproduce.py from an empty temporary working directory via `agent-lab run`. Add `--keep-visible` for the control. The script asserts private runtime, uses GTK_A11Y=atspi, owns and terminates registry/child, and writes only diagnostic logs. The caller must stop the lab afterward.

Only the named private original core and our own native fixture core were inspected. Parent release was frozen at `/tmp/koett-a11y-diagnosis.KKls9w/koett-frozen` before any possible execution, hash recorded; executing it proved unnecessary. No production source or existing script edits, installs, cloud calls, host UI/audio/settings access, commits or pushes. Selected evidence contains no audio, databases, model cache or core bytes. Original app log, registry log, accessibility snapshot and failure screenshot preserved here. Extracted core was removed from temporary storage after inspection. Private lab was stopped; owned processes exited. Fixture lint and compile checks recorded separately.

## Sources

https://github.com/GNOME/gtk/blob/4.24.1/gtk/a11y/gtkatspiroot.c

https://github.com/GNOME/gtk/blob/4.24.1/gtk/a11y/gtkatspicontext.c

Downloaded raw source SHA-256 hashes are in versions.txt; source downloads retained only in temporary storage. Installed ABI receipts are gtk-handler-abi.txt and gio-dispatch-abi.txt. Original core memory proof is core-null-path.txt.

## Parent upstream investigation

An exact upstream correction exists in commit
`21d3eb76b68d6758122eddaf00b5823f9b2d7f34` (2026-10-06),
“a11y: Return error for getChildAtIndex on the root”, fixing GTK issue #8444
and merged through MR10440. It tracks only a matched visible child and returns
a D-Bus invalid-argument error when none exists. This matches the independently
reproduced defect. Upstream main contains the correction; the official 4.24
source directory still lists 4.24.1 (2026-10-01) as latest. The private backport now passes 20 regression cycles and a real Koett journey.
See [the fix receipt](../gtk-accessibility-fix-20261030/REPORT.md). System GTK and
the installed app are unchanged; app-scoped installation remains a packaging gate.

- https://github.com/GNOME/gtk/commit/21d3eb76b68d6758122eddaf00b5823f9b2d7f34
- https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/10440
- https://download.gnome.org/sources/gtk/4.24/
