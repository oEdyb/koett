# Current-PC private candidate

This is a CachyOS x86-64 candidate, not a public Linux release or Ubuntu 22
portable runtime. The app includes patched GTK, statically linked local ASR and the
verified CPU llama.cpp b11981 runtime. Models are not included or downloaded
by this installer. Ordinary dictation stays local; Raw remains the default.

Run `python3 preflight.py` before installing. This reports host loader failures
and optional command paths without reading settings, history, keys or models.
Run `./install.sh --prefix '/private/path with spaces'` for an isolated install.
The default prefix is ~/.local. Python 3 and the current host native libraries
are prerequisites. Installation never launches or stops Koett. Existing user
configuration is retained. Version trees stay under PREFIX/lib/koett/versions;
PREFIX/bin/koett changes atomically. An old regular binary is copied beside it
as koett.previous-DIGEST. Previous version trees are retained for manual rollback.
An already-running process continues using its existing version tree.
Prefixes containing an equals sign are rejected before installation.

The launcher uses exec, forwards every argument and inherited activation token,
and sets library/resource paths only for its child process. Do not export these
paths globally. Optional llama-server gets a separate scoped loader path.

Rich Ask requires the distribution's **complete WebKitGTK 6** package, including
its subprocess executables, sandbox support and resources. This candidate does
not bundle the prior experimental mount-namespace WebKit extraction. Without
WebKitGTK 6 the native panel falls back to plain text. Install the full package
and restart Koett to enable rich rendering; never disable the WebKit sandbox.
Ask also needs a Secret Service and secret-tool for optional provider credentials;
GStreamer capture/conversion/JPEG plugins support optional screen context.
Provider calls require explicit provider configuration; dictation needs no account.
Media uses ~/.local/bin/yt-dlp and /usr/bin/ffmpeg. Wayland clipboard uses wl-copy;
wtype paste and layer-shell depend on compositor support. X11 requires its
corresponding host tooling. Preflight availability is not an end-to-end guarantee.

GTK LGPL license, exact source archive, upstream patch, build helper and
provenance are retained in runtime/gtk/share/licenses and sources. The app-scoped
GTK library can be replaced independently. ASR build provenance and llama.cpp's
MIT license are included. Aggregate native/dependency licensing and distribution
compatibility still require review before any public release.
