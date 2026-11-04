# Move the Linux recording pill

On layer-shell desktops, drag the pill body to move it to any monitor. The
compositor carries the pill preview during the drag; the real layer surface
moves through output selection and margins when dropped. Buttons keep their
normal click actions. No keyboard focus is requested and audio keeps recording.
Double-click the body to restore bottom-center on its current monitor.

The output connector and normalized on-screen position are saved separately in
`~/.config/koett/pill-position.json`, preserving settings and transcript data.
The position is restored across recordings/restarts and clamped to available
monitor space. If the saved output is absent, the first available output is used.
The existing rendering, width and 48 px row remain unchanged. Native Mac currently
has no equivalent drag behavior. GNOME positioning is separate integration work.

The implementation uses GTK's compositor-managed drag icon and temporary
keyboard-none drop surfaces for each monitor, avoiding guessed global pointer
coordinates and moving the source surface while its pointer grab is active.
Drag offers carry a random token, not clipboard contents. Temporary surfaces
are destroyed when the drag ends or is cancelled.

Validation: release build and formatting/diff checks. Interactive and private-lab
acceptance were deferred for this change; verify body drag, cross-output drop,
button clicks, reset and restart restoration on the target compositor.
