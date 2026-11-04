"""Read only the private KWin framebuffer through its screenshot API."""

import os
import sys
import threading
from pathlib import Path

import dbus
from PIL import Image

runtime = os.environ.get("AGENT_LAB_RUNTIME", "")
if runtime != os.environ.get("XDG_RUNTIME_DIR") or not runtime.startswith(
    f"/run/user/{os.getuid()}/agent-lab/"
):
    raise RuntimeError("Screenshot requires an isolated agent-lab environment")
bus = dbus.SessionBus()
pid = int(
    bus.get_object(
        "org.freedesktop.DBus", "/org/freedesktop/DBus"
    ).GetConnectionUnixProcessID("org.kde.KWin", dbus_interface="org.freedesktop.DBus")
)
name = Path(runtime).name
if name != "koett-hotplug-sol" or os.environ.get("WAYLAND_DISPLAY") != "wayland-hotplug-sol" or "/agent-lab-koett-hotplug-sol-client-" not in Path(f"/proc/{pid}/cgroup").read_text():
    raise RuntimeError("Refusing screenshot from a compositor outside this lab")
output = Path(sys.argv[1])
if output.exists():
    raise FileExistsError(output)
read_fd, write_fd = os.pipe()
chunks = []


def read_pixels():
    with os.fdopen(read_fd, "rb") as stream:
        while chunk := stream.read(65536):
            chunks.append(chunk)
            if sum(map(len, chunks)) > 64 * 1024 * 1024:
                raise RuntimeError("Lab screenshot exceeds 64 MiB")


thread = threading.Thread(target=read_pixels, daemon=True)
thread.start()
try:
    reply = bus.get_object(
        "org.kde.KWin", "/org/kde/KWin/ScreenShot2"
    ).CaptureActiveScreen(
        dbus.Dictionary({}, signature="sv"),
        dbus.types.UnixFd(write_fd),
        dbus_interface="org.kde.KWin.ScreenShot2",
        timeout=15,
    )
finally:
    os.close(write_fd)
thread.join(timeout=15)
if thread.is_alive():
    raise RuntimeError("Lab screenshot pipe did not close")
width, height, stride = (int(reply[k]) for k in ["width", "height", "stride"])
if int(reply["format"]) not in [4, 5, 6, 16, 17, 18]:
    raise RuntimeError(f"Unsupported framebuffer format: {reply['format']}")
pixels = b"".join(chunks)
if len(pixels) != height * stride:
    raise RuntimeError("Incomplete framebuffer")
order = "RGBA" if int(reply["format"]) >= 16 else "BGRA"
Image.frombuffer("RGBA", (width, height), pixels, "raw", order, stride, 1).save(output)
print(f"Saved private desktop screenshot: {output}")
