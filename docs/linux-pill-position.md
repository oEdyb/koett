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

The drag updates the layer surface's margins from the press's implicit pointer
grab. It is not Wayland drag-and-drop: the first version used that, which showed a
ghost image while the pill stayed behind, needed a drop surface on every monitor,
disturbed focus, and dropped nothing on niri.

Offsets mean different things per compositor. niri (measured) and sway keep the
press's surface origin for the whole grab, so an offset is the distance from the
press. Elsewhere it is relative to the surface's current position, so it is added to
the position the compositor has confirmed, tracked one frame behind the request. The
wrong model throws the pill away from the pointer.

A layer surface can't change output without a remap, which would end the grab. While
dragging, the pill stops at its monitor's edge; on release it jumps under the pointer
on the other monitor.
