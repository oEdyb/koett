#!/usr/bin/env python3
"""Dictionary Settings page in a private agent-lab session, through AT-SPI.

  /usr/bin/python3 scripts/test_linux_dictionary.py --binary target/debug/koett --lab koett \
      [--model PARAKEET_V2_DIR --clip SPEECH.wav --term Supabase --heard Superbase]
Start the lab first; its owner stops it. Only synthetic words and temporary
config/data directories are used. Checks add, save, reopen, export and import.
With --model, also dictates the clip through the private lab microphone twice:
without the term, then after saving it, and checks the stored transcript.
"""
from __future__ import annotations

import argparse
import ast
import json
import os
import re
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

INPUT = str(Path.home() / ".local/share/agent-lab/input")
SECTIONS = ["Audio", "Shortcuts", "Text", "Dictionary", "General"]


def wait(check, description, timeout=15):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            if result := check():
                return result
        except Exception as error:  # AT-SPI nodes disappear while windows change.
            last = error
        time.sleep(0.08)
    raise AssertionError(f"Timed out: {description}; last error: {last}")


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/koett"))
    parser.add_argument("--lab", default="koett")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--model", type=Path)
    parser.add_argument("--clip", type=Path)
    parser.add_argument("--term", default="Supabase")
    parser.add_argument("--heard", default="Superbase")
    parser.add_argument("--inside-session", action="store_true", help=argparse.SUPPRESS)
    return parser.parse_args()


def session_call(destination, path, method, *values):
    return subprocess.check_output(["gdbus", "call", "--session", "--dest", destination,
                                    "--object-path", path, "--method", method, *values], text=True)


def enable_private_accessibility(lab):
    runtime = f"/run/user/{os.getuid()}/agent-lab/{lab}"
    if (os.environ.get("AGENT_LAB_RUNTIME") != runtime or os.environ.get("XDG_RUNTIME_DIR") != runtime
            or os.environ.get("WAYLAND_DISPLAY") != "wayland-agent-lab"):
        raise SystemExit(f"Refusing to run outside private agent-lab {lab}")
    owner = session_call("org.freedesktop.DBus", "/org/freedesktop/DBus",
                         "org.freedesktop.DBus.GetConnectionUnixProcessID", "org.kde.KWin")
    compositor = int(re.search(r"uint32 ([0-9]+)", owner)[1])
    assert f"/agent-lab-{lab}.service" in Path(f"/proc/{compositor}/cgroup").read_text()
    os.environ.pop("NIRI_SOCKET", None)
    address = ast.literal_eval(session_call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus.GetAddress").strip())[0]
    os.environ["AT_SPI_BUS_ADDRESS"] = address

    def registry_ready():
        return subprocess.check_output(["gdbus", "call", "--address", address, "--dest", "org.freedesktop.DBus",
                                        "--object-path", "/org/freedesktop/DBus", "--method",
                                        "org.freedesktop.DBus.NameHasOwner", "org.a11y.atspi.Registry"],
                                       text=True).strip() == "(true,)"

    if not registry_ready():
        import atexit
        registry = next(path for path in [Path("/usr/lib/at-spi2-registryd"), Path("/usr/libexec/at-spi2-registryd")]
                        if path.exists())
        process = subprocess.Popen([str(registry)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        atexit.register(process.terminate)
        wait(registry_ready, "private accessibility registry")
    session_call("org.a11y.Bus", "/org/a11y/bus", "org.freedesktop.DBus.Properties.Set",
                 "org.a11y.Status", "IsEnabled", "<true>")
    subprocess.run(["dbus-update-activation-environment", "GTK_A11Y=atspi"], check=True)
    # File choosers come from the private GTK portal; restart it with accessibility on.
    import signal
    portal = "org.freedesktop.impl.portal.desktop.gtk"
    has_owner = lambda: session_call("org.freedesktop.DBus", "/org/freedesktop/DBus",
                                     "org.freedesktop.DBus.NameHasOwner", portal).strip() == "(true,)"
    if has_owner():
        owner = session_call("org.freedesktop.DBus", "/org/freedesktop/DBus",
                             "org.freedesktop.DBus.GetConnectionUnixProcessID", portal)
        pid = int(re.search(r"uint32 ([0-9]+)", owner)[1])
        assert f"/agent-lab-{lab}.service" in Path(f"/proc/{pid}/cgroup").read_text()
        if b"GTK_A11Y=atspi" not in Path(f"/proc/{pid}/environ").read_bytes().split(b"\0"):
            os.kill(pid, signal.SIGTERM)
            wait(lambda: not has_owner(), "private GTK portal restart")


def private_microphone(output):
    """Route ALSA capture to a remap of the lab's own audio sink monitor."""
    import atexit
    source = f"koett_dictionary_mic_{os.getpid()}"
    module = subprocess.check_output(["pactl", "load-module", "module-remap-source",
                                      "master=agent_lab_audio.monitor", f"source_name={source}"], text=True).strip()
    atexit.register(lambda: subprocess.run(["pactl", "unload-module", module], check=False))
    alsa = output / "alsa-private.conf"
    alsa.write_text(f'</usr/share/alsa/alsa.conf>\npcm.!default {{ type pulse device "{source}" }}\n'
                    'ctl.!default { type pulse }\n')
    os.environ["ALSA_CONFIG_PATH"] = str(alsa)
    os.environ["PULSE_SOURCE"] = source


def main():
    args = arguments()
    output = (args.output or Path(tempfile.mkdtemp(prefix="koett-dictionary-test-"))).resolve()
    output.mkdir(parents=True, exist_ok=True, mode=0o700)
    if not args.inside_session:
        lab = shutil.which("agent-lab") or str(Path.home() / ".local/bin/agent-lab")
        extra = []
        if args.model:
            extra = ["--model", str(args.model.resolve()), "--clip", str(args.clip.resolve()),
                     "--term", args.term, "--heard", args.heard]
        return subprocess.call([lab, "run", args.lab, "--", "/usr/bin/python3", str(Path(__file__).resolve()),
                                "--inside-session", "--lab", args.lab, "--binary", str(args.binary.resolve()),
                                "--output", str(output), *extra])
    enable_private_accessibility(args.lab)
    os.environ.update({
        "XDG_CONFIG_HOME": str(output / "config"), "XDG_DATA_HOME": str(output / "data"),
        "XDG_CACHE_HOME": str(output / "cache"), "XDG_STATE_HOME": str(output / "state"),
        "GTK_A11Y": "atspi", "GTK_THEME": "Adwaita:dark", "GTK_USE_PORTAL": "0",
    })
    config = output / "config/koett/settings.json"
    config.parent.mkdir(parents=True, exist_ok=True)
    (output / "config/user-dirs.dirs").write_text(f'XDG_DOCUMENTS_DIR="{output}"\n')
    config.write_text(json.dumps({
        "start_at_login": False,
        "model_directory": str(args.model or output / "missing-model"),
        "dictionary": [] if args.model else [{"term": "Koett", "sounds_like": ["cot"]}],
    }))
    if args.model:
        private_microphone(output)
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi

    log = (output / "app.log").open("w")
    process = subprocess.Popen([str(args.binary)], stdout=log, stderr=log)
    results = []

    def role(node):
        value = node.get_role_name()
        return {"push button": "button", "entry": "text"}.get(value, value)

    def walk(node):
        yield node
        for index in range(node.get_child_count()):
            if child := node.get_child_at_index(index):
                yield from walk(child)

    def application():
        if process.poll() is not None:
            raise AssertionError(f"Koett exited with {process.returncode}; see {output / 'app.log'}")
        return next((node for node in walk(Atspi.get_desktop(0))
                     if role(node) == "application" and node.get_process_id() == process.pid), None)

    def find_all(name, kind, root=None):
        root = root or application()
        return [node for node in walk(root) if role(node) == kind and node.get_name() == name
                and not node.get_state_set().contains(Atspi.StateType.DEFUNCT)
                and node.get_state_set().contains(Atspi.StateType.SHOWING)]

    def find(name, kind, root=None):
        return next(iter(find_all(name, kind, root)), None)

    def press(name, root=None):
        node = wait(lambda: find(name, "button", root), f"button {name}")
        wait(lambda: node.get_action_iface().do_action(0), f"enabled button {name}")

    def text(node):
        return Atspi.Text.get_text(node, 0, Atspi.Text.get_character_count(node))

    def screenshot(name):
        time.sleep(0.4)
        subprocess.run(["/usr/bin/python3", str(Path.home() / ".local/share/agent-lab/screenshot.py"),
                        str(output / name)], check=True, stdout=subprocess.DEVNULL)

    def open_settings():
        subprocess.run([str(args.binary), "settings"], check=True)
        window = wait(lambda: find("Settings", "frame"), "Settings window")
        combo = wait(lambda: find("Settings section", "combo box", window), "section selector")
        toggle = next((node for node in walk(combo) if role(node) == "toggle button"), None)
        if toggle is None:
            assert combo.get_selection_iface().select_child(SECTIONS.index("Dictionary"))
        else:
            assert toggle.get_action_iface().do_action(0)
            time.sleep(0.4)
            subprocess.run([INPUT, "key", "102", *(["108"] * SECTIONS.index("Dictionary")), "28"], check=True)
        wait(lambda: find("Add word", "button", window), "Dictionary page")
        return window

    def editable(dialog):
        return next((node for node in walk(dialog) if "EditableText" in node.get_interfaces()
                     and node.get_state_set().contains(Atspi.StateType.SHOWING)), None)

    def chooser_file(window, title, accept, name):
        press(title.split()[0] + "…", window)
        desktop = Atspi.get_desktop(0)
        dialog = wait(lambda: next((node for node in walk(desktop) if node.get_name() == title
                                    and role(node) in ("frame", "dialog", "file chooser")), None),
                      f"{title} chooser in the private portal")
        if editable(dialog) is None:
            time.sleep(0.5)
            subprocess.run([INPUT, "chord", "29", "38"], check=True)  # Ctrl+L shows the location field.
        field = wait(lambda: editable(dialog), f"{title} file name field")
        assert field.get_editable_text_iface().set_text_contents(name)
        screenshot(title.lower().replace(" ", "-") + ".png")
        press(accept, dialog)

    def dictate():
        database = output / "data/koett/History.sqlite3"
        wait(lambda: find("Record", "button"), "Ready to record", timeout=90)
        subprocess.run([str(args.binary), "start"], check=True)
        wait(lambda: find("Stop", "button"), "recording", timeout=20)
        time.sleep(0.5)
        subprocess.run(["pw-play", "--target", "agent_lab_audio", str(args.clip)], check=True)
        time.sleep(0.8)
        subprocess.run([str(args.binary), "stop"], check=True)
        wait(lambda: find("Record", "button"), "transcription finished", timeout=60)
        import sqlite3
        with sqlite3.connect(database) as connection:
            return connection.execute("SELECT text FROM transcripts ORDER BY created_at_ms DESC LIMIT 1").fetchone()[0]

    try:
        wait(application, "accessible Koett", timeout=30)
        if args.model:
            before = dictate()
            assert args.term not in before and args.heard in before, before
            settings = json.loads(config.read_text())
            settings["dictionary"] = [{"term": args.term, "sounds_like": []}]
            config.write_text(json.dumps(settings))
            time.sleep(2.5)  # The idle engine follows saved Settings within a second.
            after = dictate()
            assert args.term in after and args.heard not in after, after
            results.append(f"real dictation: '{before}' became '{after}' after saving {args.term}")
            print(json.dumps({"checks": results, "artifacts": str(output)}, indent=2))
            (output / "results.json").write_text(json.dumps(results, indent=2))
            return 0
        window = open_settings()
        word = wait(lambda: find("Word", "text", window), "existing word")
        assert text(word) == "Koett" and text(find("Sounds like", "text", window)) == "cot"
        results.append("saved dictionary loads into the page")
        press("Add word", window)
        words = wait(lambda: len(find_all("Word", "text", window)) == 2 and find_all("Word", "text", window), "new row")
        wait(lambda: words[1].get_state_set().contains(Atspi.StateType.FOCUSED), "new word focused")
        assert words[1].get_editable_text_iface().set_text_contents(" Nordicrank ")
        assert find_all("Sounds like", "text", window)[1].get_editable_text_iface().set_text_contents("nordic rank,, nordic bank")
        screenshot("dictionary-edit.png")
        press("Save", window)
        expected = [{"term": "Koett", "sounds_like": ["cot"]},
                    {"term": "Nordicrank", "sounds_like": ["nordic rank", "nordic bank"]}]
        wait(lambda: json.loads(config.read_text()).get("dictionary") == expected, "trimmed dictionary saved")
        results.append("Add word focuses a new row; Save trims and persists words and forms")

        window = open_settings()
        wait(lambda: [text(node) for node in find_all("Word", "text", window)] == ["Koett", "Nordicrank"], "reopened rows")
        exported = output / "exported.txt"
        chooser_file(window, "Export dictionary", "Export", exported.name)
        wait(lambda: exported.exists() and "Nordicrank = nordic rank, nordic bank" in exported.read_text(), "export file")
        results.append("Export writes the plain-text format through the native chooser")

        incoming = output / "incoming.txt"
        incoming.write_text("# synthetic\nKoett = quett\nGöteborg = yeah te bori\n")
        chooser_file(window, "Import dictionary", "Import", incoming.name)
        wait(lambda: [text(node) for node in find_all("Word", "text", window)] == ["Koett", "Nordicrank", "Göteborg"], "imported rows")
        assert text(find_all("Sounds like", "text", window)[0]) == "cot, quett"
        wait(lambda: any(role(node) == "label" and node.get_name().startswith("Imported 2 words") for node in walk(window)), "import notice")
        screenshot("dictionary-imported.png")
        press("Save", window)
        wait(lambda: [entry["term"] for entry in json.loads(config.read_text())["dictionary"]] == ["Koett", "Nordicrank", "Göteborg"], "imported words saved")
        results.append("Import merges new words and forms, then Save persists them")

        window = open_settings()
        press("Add word", window)
        wait(lambda: len(find_all("Word", "text", window)) == 4, "empty row")
        assert find_all("Sounds like", "text", window)[3].get_editable_text_iface().set_text_contents("orphan")
        press("Save", window)
        wait(lambda: any(role(node) == "label" and "Enter a word" in node.get_name() for node in walk(window)), "validation error")
        assert len(json.loads(config.read_text())["dictionary"]) == 3
        results.append("a sounds-like form without a word blocks Save with a visible error")
        print(json.dumps({"checks": results, "artifacts": str(output)}, indent=2))
        (output / "results.json").write_text(json.dumps(results, indent=2))
    except Exception:
        screenshot("failure.png")
        if root := application():
            (output / "accessibility.json").write_text(json.dumps(
                [{"name": node.get_name(), "role": node.get_role_name()} for node in walk(root)], indent=1))
        raise
    finally:
        if process.poll() is None:
            subprocess.run([str(args.binary), "quit"], check=False)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=5)
        log.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
