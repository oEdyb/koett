#!/usr/bin/python3
"""Logs niri's focus and layout events with times, for measuring pin flicker."""
import json, os, socket, sys, time
sock = socket.socket(socket.AF_UNIX); sock.connect(os.environ["NIRI_SOCKET"])
sock.sendall(b'"EventStream"\n')
log = open(sys.argv[1], "a", buffering=1)
for line in sock.makefile():
    event = json.loads(line)
    if "WindowFocusChanged" in event or "WorkspaceActivated" in event or "WindowLayoutsChanged" in event or "WorkspaceActiveWindowChanged" in event:
        log.write(f"{time.monotonic():.3f} {json.dumps(event)[:200]}\n")
