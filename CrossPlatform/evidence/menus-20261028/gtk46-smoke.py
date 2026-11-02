import json
import os
import subprocess
import time
from pathlib import Path

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi

assert (
    os.environ["DISPLAY"].startswith(":")
    and os.environ["XDG_RUNTIME_DIR"] == "/tmp/private-runtime"
)
subprocess.run(
    [
        "gdbus",
        "call",
        "--session",
        "--dest",
        "org.a11y.Bus",
        "--object-path",
        "/org/a11y/bus",
        "--method",
        "org.freedesktop.DBus.Properties.Set",
        "org.a11y.Status",
        "IsEnabled",
        "<true>",
    ],
    check=True,
)
root = Path("/evidence")
config = root / "config/koett/settings.json"
config.parent.mkdir(parents=True, exist_ok=True)
config.write_text(
    json.dumps({"start_at_login": False, "model_directory": "/missing-model"})
)
env = {
    **os.environ,
    "XDG_CONFIG_HOME": str(root / "config"),
    "XDG_DATA_HOME": str(root / "data"),
    "XDG_CACHE_HOME": str(root / "cache"),
    "XDG_STATE_HOME": str(root / "state"),
    "GTK_A11Y": "atspi",
    "GSK_RENDERER": "cairo",
    "XDG_SESSION_TYPE": "x11",
    "GDK_BACKEND": "x11",
}
log = (root / "app.log").open("w")
app = subprocess.Popen(["/target/release/koett"], env=env, stdout=log, stderr=log)


def walk(node):
    yield node
    for i in range(node.get_child_count()):
        child = node.get_child_at_index(i)
        if child:
            yield from walk(child)


def find(name, role=None):
    return next(
        (
            n
            for n in walk(Atspi.get_desktop(0))
            if n.get_name() == name and (role is None or n.get_role_name() == role)
        ),
        None,
    )


def wait(check):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        assert app.poll() is None, "App exited"
        time.sleep(0.1)
    (root / "accessibility.json").write_text(
        json.dumps(
            [(n.get_name(), n.get_role_name()) for n in walk(Atspi.get_desktop(0))],
            indent=2,
        )
    )
    raise AssertionError("Native menu wait timed out")


def action(name, role=None):
    n = wait(lambda: find(name, role))
    assert n.get_action_iface().do_action(0), name
    time.sleep(0.3)


def menu():
    action("Menu", "push button")
    wait(lambda: find("Hide window", "menu item"))


try:
    wait(lambda: find("Retry"))
    menu()
    names = [
        n.get_name()
        for n in walk(Atspi.get_desktop(0))
        if n.get_role_name() == "menu item"
    ]
    assert names == [
        "History",
        "Copy last transcript",
        "Settings",
        "Hide window",
        "Quit Koett",
    ], names
    assert (
        not find("Copy last transcript", "menu item")
        .get_state_set()
        .contains(Atspi.StateType.SENSITIVE)
    )
    action("Settings", "menu item")
    wait(lambda: find("Settings", "frame"))
    action("Save")
    time.sleep(0.3)
    menu()
    action("Hide window", "menu item")
    time.sleep(0.3)
    subprocess.run(["/target/release/koett", "show"], env=env, check=True)
    wait(lambda: find("Menu", "push button"))
    menu()
    action("Quit Koett", "menu item")
    app.wait(timeout=8)
    assert app.returncode == 0
    (root / "results.json").write_text(
        json.dumps(
            {
                "GTK": "4.6.9",
                "menu_accessible_names": names,
                "checks": [
                    "native menu named rows and disabled Copy",
                    "Settings and Save",
                    "Hide and show",
                    "native menu Quit",
                ],
            },
            indent=2,
        )
    )
    print("GTK4.6 native menu interaction PASS")
finally:
    if app.poll() is None:
        app.terminate()
        app.wait(timeout=8)
