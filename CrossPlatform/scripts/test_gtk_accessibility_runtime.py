#!/usr/bin/env python3
"""Verify GTK root child retirement in a private desktop with accessibility enabled."""

import argparse
import ast
import hashlib
import json
import os
import re
import select
import subprocess
import sys
import time
from pathlib import Path


def gtk_child():
    import gi

    gi.require_version("Gtk", "4.0")
    from gi.repository import GLib, Gtk

    Gtk.init()
    hidden = Gtk.Window(title="Hidden main fixture")
    visible = Gtk.Window(title="Settings fixture")
    visible.present()
    loop = GLib.MainLoop()

    def command(_source, _condition):
        nonlocal visible
        action = sys.stdin.readline().strip()
        if action == "destroy":
            visible.destroy()
            visible = None
        elif action == "present":
            visible = Gtk.Window(title="Settings fixture")
            visible.present()
        elif action == "empty":
            hidden.destroy()
        elif action == "quit":
            print(action, flush=True)
            loop.quit()
            return False
        GLib.idle_add(lambda: (print(action, flush=True), False)[1])
        return True

    GLib.io_add_watch(sys.stdin, GLib.IO_IN, command)
    GLib.idle_add(lambda: (print("ready", flush=True), False)[1])
    loop.run()


def call(address, destination, path, method, *args):
    return subprocess.run(
        ["gdbus", "call", "--address", address, "--dest", destination,
         "--object-path", path, "--method", method, "--", *args],
        capture_output=True, text=True, timeout=5, check=False,
    )


def reply(process, expected):
    assert select.select([process.stdout], [], [], 8)[0], expected
    assert process.stdout.readline().strip() == expected, expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lab", required=True)
    parser.add_argument("--library", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--rounds", type=int, default=20)
    args = parser.parse_args()
    assert re.fullmatch(r"[a-z][a-z0-9-]{0,31}", args.lab)
    runtime = f"/run/user/{os.getuid()}/agent-lab/{args.lab}"
    assert os.environ.get("AGENT_LAB_RUNTIME") == runtime
    assert os.environ.get("XDG_RUNTIME_DIR") == runtime
    assert os.environ.get("WAYLAND_DISPLAY") == "wayland-agent-lab"
    assert 1 <= args.rounds <= 100
    args.output.mkdir(parents=True, exist_ok=False)
    library = args.library.resolve(strict=True)
    address = ast.literal_eval(subprocess.check_output([
        "gdbus", "call", "--session", "--dest", "org.a11y.Bus", "--object-path",
        "/org/a11y/bus", "--method", "org.a11y.Bus.GetAddress"], text=True))[0]
    environment = {**os.environ, "GTK_A11Y": "atspi", "AT_SPI_BUS_ADDRESS": address,
                   "LD_LIBRARY_PATH": str(args.library.absolute().parent)}
    process = None
    checks = []
    with (args.output / "registry.log").open("w") as registry_log, \
            (args.output / "gtk.log").open("w") as gtk_log:
        registry = subprocess.Popen(["/usr/lib/at-spi2-registryd"], env=environment,
                                    stdout=registry_log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 8
            while True:
                owner = call(address, "org.freedesktop.DBus", "/org/freedesktop/DBus",
                             "org.freedesktop.DBus.GetNameOwner", "org.a11y.atspi.Registry")
                if owner.returncode == 0:
                    break
                assert time.monotonic() < deadline, owner.stderr
                time.sleep(0.05)
            owner_name = ast.literal_eval(owner.stdout)[0]
            registry_pid = call(address, "org.freedesktop.DBus", "/org/freedesktop/DBus",
                                "org.freedesktop.DBus.GetConnectionUnixProcessID", owner_name)
            assert ast.literal_eval(registry_pid.stdout.replace("uint32 ", ""))[0] == registry.pid
            process = subprocess.Popen([sys.executable, __file__, "--gtk-child"], env=environment,
                                       stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=gtk_log, text=True)
            reply(process, "ready")
            maps = Path(f"/proc/{process.pid}/maps").read_text()
            assert str(library) in maps, "candidate GTK was not loaded"
            deadline = time.monotonic() + 8
            name = None
            while name is None:
                names = call(address, "org.freedesktop.DBus", "/org/freedesktop/DBus",
                             "org.freedesktop.DBus.ListNames")
                for candidate in ast.literal_eval(names.stdout)[0]:
                    if candidate.startswith(":"):
                        pid = call(address, "org.freedesktop.DBus", "/org/freedesktop/DBus",
                                   "org.freedesktop.DBus.GetConnectionUnixProcessID", candidate)
                        if pid.returncode == 0 and ast.literal_eval(pid.stdout.replace("uint32 ", ""))[0] == process.pid:
                            name = candidate
                            break
                assert time.monotonic() < deadline, "GTK accessibility connection"
            path = "/org/a11y/atspi/accessible/root"
            method = "org.a11y.atspi.Accessible.GetChildAtIndex"

            def query(index, valid):
                result = call(address, name, path, method, str(index))
                if valid:
                    assert result.returncode == 0, result.stderr
                    bus, child_path = ast.literal_eval(result.stdout.replace("objectpath ", ""))[0]
                    assert bus == name and child_path.startswith("/"), result.stdout
                    assert child_path != "/org/a11y/atspi/null"
                    title = call(address, bus, child_path, "org.freedesktop.DBus.Properties.Get",
                                 "org.a11y.atspi.Accessible", "Name")
                    assert title.returncode == 0, title.stderr
                    assert ast.literal_eval(title.stdout.replace("<", "").replace(">", ""))[0] == "Settings fixture"
                else:
                    assert result.returncode != 0, result.stdout
                    assert f"No child with index {index}" in result.stderr, result.stderr
                assert process.poll() is None, "GTK process died"
                assert registry.poll() is None, "registry died"

            def command(value):
                process.stdin.write(value + "\n")
                process.stdin.flush()
                reply(process, value)

            for round_index in range(args.rounds):
                query(0, True)
                for index in [-1, 1, 999]:
                    query(index, False)
                command("destroy")
                for index in [-1, 0, 999]:
                    query(index, False)
                checks.append({"round": round_index, "valid_child": True, "retired_child_errors": True})
                if round_index + 1 < args.rounds:
                    command("present")
            command("empty")
            query(0, False)
            command("quit")
            assert process.wait(timeout=5) == 0
            receipt = {"library": str(library), "sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
                       "registry_pid": registry.pid, "rounds": checks, "empty_root_error": True,
                       "clean_exit": True}
            (args.output / "results.json").write_text(json.dumps(receipt, indent=2) + "\n")
            print(f"PASS: {len(checks)} visible/retired-child rounds; clean exit", flush=True)
        finally:
            for child in [process, registry]:
                if child is not None and child.poll() is None:
                    child.terminate()
                    child.wait(timeout=5)


if __name__ == "__main__":
    if sys.argv[1:] == ["--gtk-child"]:
        gtk_child()
    else:
        main()
