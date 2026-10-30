# Linux system-runtime candidate

This source-built candidate uses the distribution's GTK, ALSA, glibc and other
ELF libraries. It bundles no GTK or unused static ASR archives. ASR is linked
into the executable. CI builds on Ubuntu 22.04 on x86_64 and ARM64 separately.
GTK 4.6 is the compiled API floor; broad desktop compatibility is not proven.
Python 3.10+, ldd and the host loader dependencies are required.

Extract the archive, run `./koett --check-runtime`, then
`./install.sh --prefix '/your/private prefix with $dollar'`.
The default prefix is ~/.local. Preflight must pass before installation changes
anything; missing libraries cause a nonzero exit, with loader details. It does
not install packages. On Ubuntu 22.04 required runtime packages include
libgtk-4-1, libasound2, libudev1, libxkbcommon0, libwayland-client0,
libx11-6, libxcb-shape0 and libxcb-xfixes0. The actual ELF loader report is
mandatory even when those package names are installed.
Run PREFIX/bin/koett to launch. `menu-help` returns JSON without opening UI.
Installation changes only the prefix, never stops an existing process and
retains previous versions and regular-binary backups. It downloads no models.
Prefixes containing `=` are rejected. Keep all extracted files together.

System GTK 4.24.1 has a known accessibility crash fixed upstream by commit
21d3eb76b68d6758122eddaf00b5823f9b2d7f34. This flavor does not patch system GTK;
use a distribution build containing that fix or an independently validated GTK
version. Accessibility remains enabled. Passing preflight proves loader and
version availability, not desktop accessibility, audio, portals or inference.

Raw dictation needs no llama runtime. Optional S1-mini formatting needs
llama-server on PATH and a separately downloaded verified model. Optional media
uses ~/.local/bin/yt-dlp and /usr/bin/ffmpeg. Wayland clipboard uses wl-copy;
wtype depends on compositor support. Ask keys need secret-tool and a session
Secret Service. Rich Ask needs the complete distribution WebKitGTK 6 package,
including subprocess helpers/resources; plain Ask remains available otherwise.
Never disable the WebKit sandbox. See preflight's optional dependency report.
No model, provider key, user history or configuration is included.
