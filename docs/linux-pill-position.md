# Move the Linux recording pill

On layer-shell desktops, drag the pill body and the real pill follows the pointer.
Buttons keep their normal click actions, a press that moves under 4 px stays a
click, no keyboard focus is requested and audio keeps recording. Double-click the
body to restore bottom-center on its current monitor.

The output connector and normalized on-screen position are saved separately in
`~/.config/koett/pill-position.json`, preserving settings and transcript data.
The position is restored across recordings and restarts and clamped to the monitor.
If the saved output is absent, the first available output is used. Native Mac has
no equivalent drag behavior. GNOME positioning is separate integration work.

A drag keeps the real pill in place and invisible, because it holds the press's
implicit pointer grab, and shows a live copy that follows the pointer onto any
monitor. On release the real pill moves there. It is not Wayland drag-and-drop: the
first version used that, which showed a ghost while the pill stayed behind, needed a
drop surface on every monitor, disturbed focus, and dropped nothing on niri.

The real pill doesn't move during a drag, for two reasons. Pointer offsets are
measured against the surface, and compositors differ once it moves: niri keeps the
press's origin, while KWin follows the surface a variable time later, which made
margin-based dragging overshoot or oscillate. And a layer surface can't change output
without a remap, which ends the grab, which is why a moving pill stuck at the edge of
its monitor.

Checked in private labs on niri (one output) and KWin (two outputs): `same-output`,
crossing and returning, drop, second drag and restart restore, with one recording dot
within 3 px of the pointer each time.
