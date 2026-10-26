#!/usr/bin/env python3
"""Real GTK/AT-SPI smoke tests in the already-running private agent-lab koett.

Run with the system Python that provides GI/Atspi:
  /usr/bin/python3 scripts/test_linux_ui.py --binary target/debug/koett --model PATH
Start agent-lab koett separately; its owner must stop it after testing.
Add --record to exercise the private lab microphone (records two seconds).
--inside-session is reserved for lab calls or isolated Xvfb CI/container sessions.
The synthetic archive is explicitly labeled. No test-only app hooks are used.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile
import time


def wait(check, description, timeout=15):
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        try:
            result = check()
            if result:
                return result
        except Exception as error:
            last_error = error
        time.sleep(0.08)
    raise AssertionError(f"Timed out: {description}; last error: {last_error}")


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/koett"))
    parser.add_argument("--model", type=Path)
    parser.add_argument("--record", action="store_true")
    parser.add_argument("--theme", choices=["light", "dark"], default="dark")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--inside-session", action="store_true", help=argparse.SUPPRESS)
    return parser.parse_args()


def main():
    args = arguments()
    if not args.inside_session:
        # The caller owns lab startup/cleanup. Never connect to the host desktop,
        # audio graph, clipboard or accessibility bus, even when run from it.
        lab = shutil.which("agent-lab") or str(Path.home() / ".local/bin/agent-lab")
        output = (args.output or Path(tempfile.mkdtemp(prefix="koett-ui-test-"))).resolve()
        output.mkdir(parents=True, exist_ok=True, mode=0o700)
        command = [lab, "run", "koett", "--", "/usr/bin/python3", str(Path(__file__).resolve()),
                   "--inside-session", "--binary", str(args.binary.resolve()),
                   "--output", str(output), "--theme", args.theme]
        if args.model:
            command.extend(["--model", str(args.model.resolve())])
        if args.record:
            command.append("--record")
        print("Using existing agent-lab koett; caller owns lab cleanup.", flush=True)
        return subprocess.call(command)

    # Explicit inner mode also supports isolated Xvfb CI and containers. Lab
    # routing supplies its own runtime/session/audio; only test state is replaced.
    output = (args.output or Path(tempfile.mkdtemp(prefix="koett-ui-test-"))).resolve()
    output.mkdir(parents=True, exist_ok=True, mode=0o700)
    lab_runtime = os.environ.get("AGENT_LAB_RUNTIME")
    os.environ.pop("NIRI_SOCKET", None)
    if os.environ.get("WAYLAND_DISPLAY"):
        expected_runtime = f"/run/user/{os.getuid()}/agent-lab/koett"
        if (lab_runtime != expected_runtime
                or os.environ.get("XDG_RUNTIME_DIR") != expected_runtime
                or os.environ.get("WAYLAND_DISPLAY") != "wayland-agent-lab"
                or not os.environ.get("DBUS_SESSION_BUS_ADDRESS")
                or not os.environ.get("AGENT_LAB_GENERATION")):
            raise SystemExit("Refusing Wayland outside private agent-lab koett")
        # Resolve accessibility exclusively through the private session bus.
        os.environ.pop("AT_SPI_BUS_ADDRESS", None)
    elif not os.environ.get("DISPLAY"):
        raise SystemExit("--inside-session requires agent-lab or isolated Xvfb")
    else:
        display = os.environ["DISPLAY"].split(".", 1)[0]
        virtual_display = False
        for process_dir in Path("/proc").iterdir():
            if not process_dir.name.isdigit():
                continue
            try:
                command = (process_dir / "cmdline").read_bytes().split(b"\0")
                if command and Path(os.fsdecode(command[0])).name == "Xvfb":
                    virtual_display |= os.fsencode(display) in command
            except (FileNotFoundError, PermissionError):
                continue
        if not virtual_display:
            raise SystemExit("Refusing X11 outside an isolated Xvfb display")
    os.environ.update({
        "XDG_CONFIG_HOME": str(output / "config"),
        "XDG_DATA_HOME": str(output / "data"),
        "XDG_CACHE_HOME": str(output / "cache"),
        "XDG_STATE_HOME": str(output / "state"),
        "GTK_A11Y": "atspi",
        "GTK_THEME": "Adwaita:" + args.theme,
        "GTK_USE_PORTAL": "0",
    })
    if lab_runtime:
        import ast
        import re
        import signal
        def session_call(destination, path, method, *values):
            return subprocess.check_output(["gdbus", "call", "--session", "--dest", destination,
                "--object-path", path, "--method", method, *values], text=True)

        owner = session_call("org.freedesktop.DBus", "/org/freedesktop/DBus",
                             "org.freedesktop.DBus.GetConnectionUnixProcessID", "org.kde.KWin")
        compositor = int(re.search(r"uint32 ([0-9]+)", owner)[1])
        assert "/agent-lab-koett.service" in Path(f"/proc/{compositor}/cgroup").read_text()
        address = ast.literal_eval(session_call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus.GetAddress").strip())[0]
        os.environ["AT_SPI_BUS_ADDRESS"] = address

        def registry_ready():
            return subprocess.check_output(["gdbus", "call", "--address", address,
                "--dest", "org.freedesktop.DBus", "--object-path", "/org/freedesktop/DBus",
                "--method", "org.freedesktop.DBus.NameHasOwner", "org.a11y.atspi.Registry"], text=True).strip() == "(true,)"

        if not registry_ready():
            import atexit
            registry = next(path for path in [Path("/usr/lib/at-spi2-registryd"), Path("/usr/libexec/at-spi2-registryd")] if path.exists())
            registry_process = subprocess.Popen([str(registry)], stdout=(output / "accessibility.log").open("w"), stderr=subprocess.STDOUT)
            def stop_registry():
                if registry_process.poll() is None:
                    registry_process.terminate()
                    try:
                        registry_process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        registry_process.kill()
                        registry_process.wait(timeout=3)
            atexit.register(stop_registry)
            wait(registry_ready, "private accessibility registry")
        session_call("org.a11y.Bus", "/org/a11y/bus", "org.freedesktop.DBus.Properties.Set", "org.a11y.Status", "IsEnabled", "<true>")
        subprocess.run(["dbus-update-activation-environment", "GTK_A11Y"], check=True)
        gtk_portal = "org.freedesktop.impl.portal.desktop.gtk"
        if session_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus.NameHasOwner", gtk_portal).strip() == "(true,)":
            owner = session_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus.GetConnectionUnixProcessID", gtk_portal)
            pid = int(re.search(r"uint32 ([0-9]+)", owner)[1])
            assert "/agent-lab-koett.service" in Path(f"/proc/{pid}/cgroup").read_text()
            if b"GTK_A11Y=atspi" not in Path(f"/proc/{pid}/environ").read_bytes().split(b"\0"):
                os.kill(pid, signal.SIGTERM)
                wait(lambda: session_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus.NameHasOwner", gtk_portal).strip() == "(false,)", "private GTK portal restart")

    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi

    config = output / "config/koett/settings.json"
    config.parent.mkdir(parents=True, exist_ok=True)
    settings = {"start_at_login": False, "model_directory": str(output / "missing-model")}
    config.write_text(json.dumps(settings))
    (output / "config/user-dirs.dirs").write_text(f'XDG_DOCUMENTS_DIR="{output}"\n')
    database = output / "data/koett/History.sqlite3"
    database.parent.mkdir(parents=True, exist_ok=True)
    schema = Path(__file__).resolve().parents[2] / "Sources/Koett/Resources/History/schema.sql"
    texts = ["Synthetic UI fixture alpha: full transcript.\n\nUnicode needle: åäö 日本語.",
             "Synthetic UI fixture beta: second selected transcript."]
    with sqlite3.connect(database) as connection:
        connection.executescript(schema.read_text())
        connection.execute("DELETE FROM transcripts WHERE model='Synthetic UI fixture'")
        for index in range(65):
            text = texts[index] if index < len(texts) else f"Synthetic UI fixture older entry {index}."
            connection.execute(
                "INSERT INTO transcripts(id,created_at_ms,model,text,raw_text) VALUES(?,?,?,?,NULL)",
                (f"fixture-{index:03}", 1_793_000_000_000 - index * 60_000, "Synthetic UI fixture", text),
            )
    logfile = (output / "app.log").open("w")
    process = subprocess.Popen([str(args.binary)], stdout=logfile, stderr=logfile)
    results = []
    clipboard_original = None
    clipboard_owned = None
    if shutil.which("wl-copy") and shutil.which("wl-paste") and os.environ.get("WAYLAND_DISPLAY"):
        types = subprocess.run(["wl-paste", "--list-types"], capture_output=True, timeout=2).stdout.decode().splitlines()
        kind = next((kind for kind in types if kind.startswith("text/plain")), types[0] if types else None)
        if kind:
            original = subprocess.run(["wl-paste", "--no-newline", "--type", kind], capture_output=True, timeout=2)
            if original.returncode == 0: clipboard_original = (kind, original.stdout)
        else: clipboard_original = (None, b"")

    def node_role(node):
        value = node.get_role_name()
        if value == "push button": return "button"
        if value == "entry": return "text"
        return value

    def walk(node):
        yield node
        for index in range(node.get_child_count()):
            child = node.get_child_at_index(index)
            if child:
                yield from walk(child)

    def application():
        if process.poll() is not None:
            raise AssertionError(f"Koett exited with {process.returncode}; see {output / 'app.log'}")
        desktop = Atspi.get_desktop(0)
        return next((node for node in walk(desktop)
                     if node_role(node) == "application" and node.get_process_id() == process.pid), None)

    def find(name, role=None, root=None):
        root = root or application()
        if root is None:
            return None
        expected = "text" if role == "entry" else role
        for node in walk(root):
            if expected is not None and node_role(node) != expected:
                continue
            states = node.get_state_set()
            if states.contains(Atspi.StateType.DEFUNCT):
                continue
            if node_role(node) == "frame" and not states.contains(Atspi.StateType.SHOWING):
                continue
            actual = node.get_name()
            if actual == name or (expected == "button" and actual == name + " " + name):
                return node
            if expected == "check box" and actual.replace("_", "") == name:
                return node
            # GTK4.6 concatenates a button's caption and external label into Name.
            # Its own visible label remains the exact caption on every GTK version.
            if expected == "button" and "Action" in node.get_interfaces() and any(
                node_role(child) == "label" and (
                    child.get_name() == name
                    or ("Text" in child.get_interfaces()
                        and Atspi.Text.get_text(child, 0, -1) == name)
                )
                for child in list(walk(node))[1:]
            ):
                return node
        return None

    def button(name, root=None):
        node = wait(lambda: find(name, "button", root), f"button {name}")
        wait(lambda: node.get_action_iface().do_action(0), f"enabled button {name}")

    def labels():
        return [node.get_name() for node in walk(application()) if node_role(node) == "label"]

    def transcript():
        for node in walk(application()):
            if node_role(node) == "text" and "Text" in node.get_interfaces() and not node.get_state_set().contains(Atspi.StateType.EDITABLE):
                return Atspi.Text.get_text(node, 0, -1)
        return None

    def listbox():
        return next(node for node in walk(application()) if node_role(node) == "list")

    def search(value):
        entry = wait(lambda: find("Search transcripts", "entry"), "named search entry")
        assert entry.get_editable_text_iface().set_text_contents(value)

    def screenshot(name):
        nonlocal clipboard_owned
        if lab_runtime:
            subprocess.run(["/usr/bin/python3", str(Path.home() / ".local/share/agent-lab/screenshot.py"),
                            str(output / name)], check=True)
            return
        # niri's scoped screenshot action does not capture unrelated windows.
        if not shutil.which("niri") or not os.environ.get("NIRI_SOCKET"):
            return
        windows = json.loads(subprocess.check_output(["niri", "msg", "-j", "windows"]))
        window = next((window for window in windows if window["pid"] == process.pid and window["title"] == "Koett"), None)
        if window:
            subprocess.run(["niri", "msg", "action", "screenshot-window", "--id", str(window["id"]),
                            "--path", str(output / name)], check=True)
            wait(lambda: (output / name).exists(), "native screenshot file")
            clipboard_owned = ("image/png", (output / name).read_bytes())
            wait(lambda: subprocess.run(["wl-paste", "--no-newline", "--type", "image/png"], capture_output=True).stdout == clipboard_owned[1], "scoped screenshot clipboard transfer")

    def mnemonic(key):
        if lab_runtime:
            if key != "p":
                raise AssertionError(f"Unsupported lab mnemonic: {key}")
            subprocess.run([str(Path.home() / ".local/share/agent-lab/input"), "chord", "56", "25"], check=True)
        elif os.environ.get("WAYLAND_DISPLAY"):
            windows = json.loads(subprocess.check_output(["niri", "msg", "-j", "windows"]))
            window = next(w for w in windows if w["pid"] == process.pid and w["title"] == "Settings")
            subprocess.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])], check=True)
            wait(lambda: json.loads(subprocess.check_output(["niri", "msg", "-j", "focused-window"]))["id"] == window["id"], "focused settings window")
            subprocess.run(["wtype", "-M", "alt", "-k", key, "-m", "alt"], check=True)
        else:
            x11_chord([0xffe9, ord(key)])

    def x11_chord(symbols):
        import ctypes
        x11 = ctypes.CDLL("libX11.so.6")
        xtest = ctypes.CDLL("libXtst.so.6")
        x11.XOpenDisplay.restype = ctypes.c_void_p
        x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        x11.XKeysymToKeycode.restype = ctypes.c_uint
        xtest.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
        x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
        x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
        display = x11.XOpenDisplay(None)
        assert display
        keycodes = [x11.XKeysymToKeycode(display, symbol) for symbol in symbols]
        for code in keycodes:
            assert xtest.XTestFakeKeyEvent(display, code, True, 0)
        for code in reversed(keycodes):
            assert xtest.XTestFakeKeyEvent(display, code, False, 0)
        x11.XSync(display, False)
        x11.XCloseDisplay(display)

    def control(command):
        subprocess.run([str(args.binary), command], check=True)

    def focus_lab_window(title):
        if not lab_runtime:
            return
        import ast
        import re
        command = ["gdbus", "call", "--session", "--dest", "org.kde.KWin"]
        owner = subprocess.check_output([
            "gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
            "--object-path", "/org/freedesktop/DBus", "--method",
            "org.freedesktop.DBus.GetConnectionUnixProcessID", "org.kde.KWin",
        ], text=True)
        pid = int(re.search(r"uint32 ([0-9]+)", owner)[1])
        assert "/agent-lab-koett.service" in Path(f"/proc/{pid}/cgroup").read_text()
        script = output / "focus-owned-window.js"
        script.write_text(f"const target = workspace.windowList().find(w => w.pid === {process.pid} && w.caption === {json.dumps(title)}); if (target) workspace.activeWindow = target;")
        plugin = f"koett-ui-{process.pid}"
        loaded = subprocess.check_output([*command, "--object-path", "/Scripting",
            "--method", "org.kde.kwin.Scripting.loadScript", str(script), plugin], text=True)
        index = ast.literal_eval(loaded.strip())[0]
        assert index >= 0
        try:
            subprocess.run([*command, "--object-path", f"/Scripting/Script{index}",
                "--method", "org.kde.kwin.Script.run"], check=True, stdout=subprocess.DEVNULL)
        finally:
            subprocess.run([*command, "--object-path", "/Scripting", "--method",
                "org.kde.kwin.Scripting.unloadScript", plugin], check=True, stdout=subprocess.DEVNULL)

    def entry_text(entry):
        return next((value for node in walk(entry) if "Text" in node.get_interfaces()
                     and (value := Atspi.Text.get_text(node, 0, Atspi.Text.get_character_count(node)))), "")

    def choose_microphone(combo, index):
        toggle = next((node for node in walk(combo) if node_role(node) == "toggle button"), None)
        if toggle is None:
            assert combo.get_selection_iface().select_child(index)
            return
        wait(lambda: toggle.get_state_set().contains(Atspi.StateType.FOCUSED)
             or toggle.get_component_iface().grab_focus(), "microphone keyboard focus")
        input_tool = str(Path.home() / ".local/share/agent-lab/input")
        subprocess.run([input_tool, "key", "57"], check=True)
        screenshot(f"microphone-{index}.png")
        subprocess.run([input_tool, "key", "102", *(["108"] * index), "28", "15"], check=True)

    try:
        wait(application, "accessible GTK application")
        if shutil.which("niri") and os.environ.get("NIRI_SOCKET"):
            def own_window():
                windows = json.loads(subprocess.check_output(["niri", "msg", "-j", "windows"]))
                return next((window for window in windows if window["pid"] == process.pid and window["title"] == "Koett"), None)
            window = wait(own_window, "native compositor window")
            if not window["is_floating"]:
                subprocess.run(["niri", "msg", "action", "toggle-window-floating", "--id", str(window["id"])], check=True)
            for dimension, size in [("width", "960"), ("height", "680")]:
                subprocess.run(["niri", "msg", "action", "set-window-" + dimension, "--id", str(window["id"]), size], check=True)
            wait(lambda: own_window()["layout"]["window_size"] == [960, 680], "960×680 native window")
        wait(lambda: any(label.startswith("Error:") for label in labels()), "failed-model status")
        wait(lambda: find("Retry", "button"), "failed-model Retry button")
        results.append("failed model exposes Retry")
        wait(lambda: transcript() == texts[0], "initial newest selection")
        assert len([node for node in walk(listbox()) if node_role(node) == "list item"]) == 50
        results.append("initial page has exactly 50 summaries and newest full text")
        button("Older transcripts")
        wait(lambda: len([node for node in walk(listbox()) if node_role(node) == "list item"]) == 15, "older page")
        button("Newest transcripts")
        wait(lambda: len([node for node in walk(listbox()) if node_role(node) == "list item"]) == 50, "newest page")
        results.append("bounded chronological paging")

        search("nonexistent-phrase-ui-fixture")
        wait(lambda: find("No matches", "label"), "distinct empty search")
        screenshot("no-matches.png")
        button("Clear search")
        wait(lambda: find("Copy", "button"), "browse after clearing search")
        search("beta")
        wait(lambda: len([node for node in walk(listbox()) if node_role(node) == "list item"]) == 1, "literal search result")
        assert listbox().get_selection_iface().select_child(0)
        wait(lambda: transcript() == texts[1], "selected full transcript")
        results.append("search, clear and explicit selection")

        if shutil.which("wl-copy") and shutil.which("wl-paste") and os.environ.get("WAYLAND_DISPLAY"):
            copy_button = wait(lambda: find("Copy", "button"), "Copy button")
            if lab_runtime:
                # Supply a real input serial on the private compositor for Copy.
                subprocess.run([str(Path.home() / ".local/share/agent-lab/input"), "chord", "29", "33"], check=True)
                button("Copy")
            elif shutil.which("niri") and shutil.which("wtype") and os.environ.get("NIRI_SOCKET"):
                windows = json.loads(subprocess.check_output(["niri", "msg", "-j", "windows"]))
                window = next(w for w in windows if w["pid"] == process.pid and w["title"] == "Koett")
                subprocess.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])], check=True)
                wait(lambda: json.loads(subprocess.check_output(["niri", "msg", "-j", "focused-window"]))["pid"] == process.pid, "focused test window")
                focused = json.loads(subprocess.check_output(["niri", "msg", "-j", "focused-window"]))
                assert focused["pid"] == process.pid
                # A real keyboard serial is required by Wayland clipboard security.
                # Ctrl+F is the app's normal search action and does not modify text.
                subprocess.run(["wtype", "-M", "ctrl", "-k", "f", "-m", "ctrl"], check=True)
                button("Copy")
            else:
                button("Copy")
            wait(lambda: subprocess.check_output(["wl-paste", "--no-newline", "--type", "text/plain"], stderr=subprocess.DEVNULL).decode() == texts[1], "exact copied text")
            clipboard_owned = ("text/plain", texts[1].encode())
            results.append("native Copy preserves exact selected text")
        else:
            results.append("SKIP clipboard: wl-copy/wl-paste and Wayland required")

        # GTK_USE_PORTAL=0 selects GTK's native chooser on the private desktop.
        button("Export…")
        chooser = wait(lambda: find("Export transcript", root=Atspi.get_desktop(0)), "named native export chooser")
        print("Export fields:", [(node.get_name(), node_role(node)) for node in walk(chooser) if "EditableText" in node.get_interfaces()], flush=True)
        name = next((node for node in walk(chooser) if ("name" in node.get_name().lower()) and "EditableText" in node.get_interfaces()), None)
        if name is None:
            name = next(node for node in walk(chooser) if "EditableText" in node.get_interfaces())
        destination = output / "exported.txt"
        assert name.get_editable_text_iface().set_text_contents(destination.name)
        button("Export", chooser)
        wait(lambda: destination.exists(), "exported file")
        assert destination.read_text() == texts[1]
        results.append("native Export chooser writes exact plain text")

        button("Settings")
        settings_window = wait(lambda: find("Settings", "frame"), "native settings")
        focus_lab_window("Settings")
        wait(lambda: settings_window.get_state_set().contains(Atspi.StateType.ACTIVE), "active Settings window")
        microphone = wait(lambda: find("Microphone", "combo box", settings_window), "labeled microphone")
        # Index 0 is System default; choose one enumerated device and verify stable ID persistence.
        choose_microphone(microphone, 1)
        saved_shortcut = "Ctrl+Shift+Space"
        if os.environ.get("WAYLAND_DISPLAY"):
            wait(lambda: any(node_role(node) == "label" and "Text" in node.get_interfaces() and Atspi.Text.get_text(node, 0, -1) == "Managed by desktop" for node in walk(settings_window)), "desktop-managed shortcut label")
            assert find("Change desktop shortcut…", "button", settings_window)
        else:
            wait(lambda: settings_window.get_state_set().contains(Atspi.StateType.ACTIVE), "active Settings window")
            button(saved_shortcut, settings_window)
            wait(lambda: find("Press your shortcut…", "button", settings_window), "shortcut recorder active")
            x11_chord([0xffe3, 0xffe9, 0x20])
            wait(lambda: find("Ctrl+Alt+Space", "button", settings_window), "captured native shortcut")
            saved_shortcut = "Ctrl+Alt+Space"
        button("Add replacement", settings_window)
        phrase = wait(lambda: find("Phrase", "text", settings_window), "labeled replacement phrase")
        replacement = find("Replacement", "text", settings_window)
        assert phrase.get_editable_text_iface().set_text_contents("koet")
        assert replacement.get_editable_text_iface().set_text_contents("Koett")
        pause = find("Pause media while recording", "check box", settings_window)
        if pause.get_action_iface().get_n_actions():
            assert pause.get_action_iface().do_action(0)
        else:
            mnemonic("p")
        wait(lambda: pause.get_state_set().contains(Atspi.StateType.CHECKED), "pause checkbox checked")
        button("Save", settings_window)
        persisted = wait(lambda: json.loads(config.read_text()).get("replacements"), "saved settings")
        assert persisted == [{"phrase": "koet", "replacement": "Koett"}]
        saved = json.loads(config.read_text())
        assert saved["shortcut"] == saved_shortcut and saved["pause_media"]
        assert saved["microphone"] is not None
        if not os.environ.get("WAYLAND_DISPLAY"):
            wait(lambda: find(saved_shortcut, "label"), "updated shortcut footer")
        results.append("native settings persist microphone ID, shortcut, replacement and media choice")
        wait(lambda: not settings_window.get_state_set().contains(Atspi.StateType.SHOWING), "saved Settings window closed")
        button("Settings")
        settings_window = wait(lambda: find("Settings", "frame"), "reopened settings")
        focus_lab_window("Settings")
        wait(lambda: (entry := find("Phrase", "text", settings_window)) and entry_text(entry) == "koet", "reopened replacement")
        microphone = find("Microphone", "combo box", settings_window)
        choose_microphone(microphone, 0)
        button("Save", settings_window)
        wait(lambda: json.loads(config.read_text())["microphone"] is None, "default input saved")
        results.append("settings reopen and system default microphone")

        if args.model:
            settings = json.loads(config.read_text())
            settings["model_directory"] = str(args.model)
            settings["pause_media"] = False
            config.write_text(json.dumps(settings))
            button("Retry")
            wait(lambda: any(label == "Ready" or label.startswith("Ready ·") for label in labels()), "real model recovery to Ready", timeout=60)
            results.append("failed-model Retry loads real model and reaches Ready")
            search("")
            wait(lambda: find("Copy", "button"), "populated Ready view")
            screenshot("ready.png")
            with sqlite3.connect(database) as connection:
                connection.execute("DELETE FROM transcripts WHERE model='Synthetic UI fixture'")
            control("show")
            wait(lambda: find("No transcripts yet", "label"), "empty history with recording action")
            copy = find("Copy", "button")
            assert find("Record", "button") and (copy is None or not copy.get_state_set().contains(Atspi.StateType.SHOWING))
            screenshot("empty.png")
            results.append("empty history has Record and no blank editor actions")
            if args.record:
                button("Record")
                wait(lambda: find("Stop", "button"), "real microphone Recording", timeout=20)
                time.sleep(2)
                screenshot("recording.png")
                button("Stop")
                wait(lambda: find("Record", "button"), "capture stop and ASR completion", timeout=60)
                results.append("real default microphone start/2s capture/stop/local ASR completion")
        else:
            results.append("SKIP real model and microphone: pass --model and --record")
        print(json.dumps({"checks": results, "artifacts": str(output)}, indent=2))
        (output / "results.json").write_text(json.dumps(results, indent=2))
    except Exception:
        dump = []
        root = application()
        if root:
            for node in walk(root):
                dump.append({"name": node.get_name(), "role": node.get_role_name(), "interfaces": node.get_interfaces()})
        (output / "accessibility.json").write_text(json.dumps(dump, indent=2))
        raise
    finally:
        if process.poll() is None:
            control("quit")
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=5)
        logfile.close()
        # Never overwrite a clipboard another application changed during these checks.
        if clipboard_original is not None and clipboard_owned is not None:
            kind, content = clipboard_owned
            current = subprocess.run(["wl-paste", "--no-newline", "--type", kind], capture_output=True)
            if current.returncode == 0 and current.stdout == content:
                original_type, original = clipboard_original
                if original_type is None:
                    subprocess.run(["wl-copy", "--clear"], check=True)
                else:
                    subprocess.run(["wl-copy", "--type", original_type], input=original, check=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
