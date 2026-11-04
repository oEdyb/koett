import os
assert os.environ.get('AGENT_LAB_RUNTIME','').endswith('/koett-shift-sol')
assert os.environ.get('KOETT_TEST_DRAW_KEYBOARD')=='Koett shift sol grabbed'
"""Virtual Shift key for Koett's device-mode test. The device is grabbed with
EVIOCGRAB before any key is sent, so the host compositor never receives its
events; only the kernel key state (read via EVIOCGKEY) changes."""
import fcntl, os, struct, sys, time

UI_SET_EVBIT, UI_SET_KEYBIT = 0x40045564, 0x40045565
UI_DEV_SETUP, UI_DEV_CREATE, UI_DEV_DESTROY = 0x405C5503, 0x5501, 0x5502
EVIOCGRAB = 0x40044590
EV_SYN, EV_KEY, KEY_LEFTSHIFT, KEY_LEFTCTRL, KEY_A = 0, 1, 42, 29, 30

ui = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
fcntl.ioctl(ui, UI_SET_EVBIT, EV_KEY)
for key in (KEY_LEFTSHIFT, KEY_LEFTCTRL, KEY_A):
    fcntl.ioctl(ui, UI_SET_KEYBIT, key)
# Koett ignores virtual keyboards unless a private lab names this one in
# KOETT_TEST_DRAW_KEYBOARD, so a host Koett never reacts to this tool.
name = (sys.argv[1] if len(sys.argv) > 1 else "Koett lab test shift (grabbed)").encode()
fcntl.ioctl(ui, UI_DEV_SETUP, struct.pack("HHHH80sI", 0x06, 0x1234, 0x5678, 1, name, 0))
fcntl.ioctl(ui, UI_DEV_CREATE)
buf = bytearray(64)
fcntl.ioctl(ui, 0x80000000 | (64 << 16) | 0x552C, buf)
sysname = buf.split(b"\0")[0].decode()
deadline = time.time() + 5
node = None
while node is None and time.time() < deadline:
    path = f"/sys/devices/virtual/input/{sysname}"
    events = [n for n in os.listdir(path) if n.startswith("event")] if os.path.isdir(path) else []
    node = f"/dev/input/{events[0]}" if events else None
    time.sleep(0.05)
deadline = time.time() + 5
while True:  # udev applies permissions just after the node appears
    try:
        grab = os.open(node, os.O_RDONLY)
        break
    except PermissionError:
        if time.time() > deadline:
            raise
        time.sleep(0.05)
fcntl.ioctl(grab, EVIOCGRAB, 1)
print("grabbed", node, flush=True)

def emit(value, key=KEY_LEFTSHIFT):
    now = time.time()
    sec, usec = int(now), int((now % 1) * 1e6)
    os.write(ui, struct.pack("llHHi", sec, usec, EV_KEY, key, value))
    os.write(ui, struct.pack("llHHi", sec, usec, EV_SYN, 0, 0))

try:
    for line in sys.stdin:
        command = line.strip()
        if command == "down":
            emit(1)
        elif command == "up":
            emit(0)
        elif command == "ctrl-down":
            emit(1, KEY_LEFTCTRL)
        elif command == "ctrl-up":
            emit(0, KEY_LEFTCTRL)
        elif command == "a-down":
            emit(1, KEY_A)
        elif command == "a-up":
            emit(0, KEY_A)
        elif command == "quit":
            break
        print("ok", command, flush=True)
finally:
    emit(0)
    fcntl.ioctl(grab, EVIOCGRAB, 0)
    os.close(grab)
    fcntl.ioctl(ui, UI_DEV_DESTROY)
    os.close(ui)
