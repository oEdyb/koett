# Verified private GTK accessibility backport

GTK 4.24.1 plus upstream commit `21d3eb76b68d6758122eddaf00b5823f9b2d7f34`
corrects the application-root child-index failure diagnosed in the adjacent
[crash report](../gtk-accessibility-crash-20261030/REPORT.md). This is the
upstream fix for issue #8444 / MR10440, not a Rust lifetime workaround.

## Inputs and build

The official GTK source archive SHA256 is
`e98abe720e16129c8c0f50761dee0a2e9ae2478055e31018f6cf977fcf6513e9`.
The exact upstream patch SHA256 is
`f2d4b12c7dcd5bd98cf20df429fac67ba764fb0dac42271f4d58eafb06840535`.
Wayland protocols 1.48 is pinned to commit
`02e63e74a807afed95bc25a386173110afef24e3`; it supplies XML/pkg-config data
inside the private build prefix. No system package was installed.

The reproducible recipe is `CrossPlatform/scripts/build_linux_gtk_runtime.py`.
It checks every cached source entry against a fresh archive-plus-patch tree,
rejects modified protocol source, verifies both Meson source directories and
private prefixes, validates requested build options and records actual options.
The two-core/four-GiB compile scope is in `agents-lab.slice`. Source input,
compiler, build options, dynamic dependencies, source-tree digest and runtime
hash are recorded in runtime-receipt.json. No toolkit binary or source archive
is tracked here. LGPL text, upstream patch and provenance accompany the private
runtime under `share/licenses/gtk4`.

The current candidate is under
`~/.cache/koett/runtime-builds/gtk-4.24.1-21d3eb7/runtime`; its historical
`/tmp/koett-gtk-upstream-fix` prefix is a symlink to that owned cache. Only this
task's build files were moved when tmpfs filled; other temporary files were
untouched. This candidate remains process-scoped, not installed as system GTK.

## Actual verification

`test_gtk_accessibility_runtime.py` passed 20 repeated visible/retired-window
cycles with GTK accessibility enabled and an owned, live registry. Each visible
reference resolves to the exact Settings fixture title; negative, out-of-range
and retired-child queries return the explicit upstream error. An empty root is
also checked, followed by a clean process exit. The fixture verifies the actual
loaded library through its own process maps. regression-results.json records
these outcomes and the library hash. Original stock GTK failures remain in the
crash report; this result does not replace or suppress them.

A frozen real Koett binary also passed the existing portal/native journey with
this exact loaded library: Settings unassigned/assigned key values, dictation
toggle, held recording/release, live rebind and Ask/media action dispatch.
The selected screenshots were inspected. koett-runtime.json and koett-results.json
record binary/library identity and assertions. Ask was deliberately disabled;
its dispatch proof is not an assistant-provider answer. ConfigureShortcuts method
invocation is proved, not editor presentation.

The runtime SHA256 agrees in all receipts:
`4c7b561bdea19407b7d795072360b1be6c5f5860d87f315580501c97d9f7d204`.
The later stateful-menu journeys also use and record this runtime; their own
receipts cover the subsequently rebuilt application.

Six build-input regression tests pass: modified and unexpected source files,
wrong cached install prefix/source, debug-vs-release mismatch, and corrupt cached
download rejection. Ruff 0.17.2 and Python compilation pass. Independent reviews
identified the cache/source/prefix/options gaps; these are fixed and tested.
Early fixture failures came from an incorrect GTK reference-path assumption,
gdbus parsing a negative argument as an option, and a quit acknowledgement queued
after loop exit. Assertions now inspect the actual accessible title and exact
error, and acknowledge quit synchronously. No product assertion was weakened.

System GTK remains SHA256
`fd50597c435753a2ac7382702e4cd107557e647c975d9d3ed12824d167edad07`,
matching the original diagnosis. The installed Koett binary and live preferences
were not changed. The candidate fixes the demonstrated defect; it is not a claim
that every GTK failure is resolved.

## Reproduction and limits

On a matching Linux development host, run:

```sh
python3 CrossPlatform/scripts/build_linux_gtk_runtime.py --work-directory "$HOME/.cache/koett/gtk-backport"
agent-lab start koett-gtk-patched
agent-lab run koett-gtk-patched -- /usr/bin/python3 CrossPlatform/scripts/test_gtk_accessibility_runtime.py --lab koett-gtk-patched --library "$HOME/.cache/koett/gtk-backport/runtime/lib/libgtk-4.so.1" --output "$HOME/.cache/koett/gtk-backport/check-new" --rounds 20
agent-lab stop koett-gtk-patched
```

The build requires the recorded native development packages and a recent Python;
it does not install prerequisites globally. Keep all GUI checks in the private
lab. The runtime is compiled against this PC's GLib/Pango/Cairo/Vulkan stack. It
is not an Ubuntu 22.04 portable runtime or a public Linux package. Installation,
loader integration and complete renderer/platform acceptance remain separate
gates. No installed launcher was changed.

## Primary sources

- https://github.com/GNOME/gtk/commit/21d3eb76b68d6758122eddaf00b5823f9b2d7f34
- https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/10440
- https://download.gnome.org/sources/gtk/4.24/gtk-4.24.1.tar.xz
- https://download.gnome.org/sources/gtk/4.24/gtk-4.24.1.sha256sum
- https://gitlab.freedesktop.org/wayland/wayland-protocols/-/tree/1.48
