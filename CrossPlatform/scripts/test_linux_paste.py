#!/usr/bin/env python3
"""Private compositor paste test using real ASR, GTK Entry and kitty consumption.

Caller owns agent-lab koett-paste startup/cleanup. Run inside its private session;
pass an empty output directory, a release binary, and the pinned model directory.
Use --niri for the nested virtual-keyboard fallback (no private portal service).
Portal permission must be granted inside the private lab when prompted.
"""

import argparse
import json
import os
import signal
import sqlite3
import subprocess
import sys
import time
from pathlib import Path


def wait(check, label, timeout=40):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        value = check()
        if value:
            return value
        time.sleep(0.1)
    raise RuntimeError("Timed out: " + label)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--niri", action="store_true")
    parser.add_argument("--nested", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    runtime = Path(f"/run/user/{os.getuid()}/agent-lab/koett-paste")
    assert os.environ.get("AGENT_LAB_RUNTIME") == str(runtime)
    assert os.environ.get("XDG_RUNTIME_DIR") == str(runtime)
    assert os.environ.get("AGENT_LAB_GENERATION")
    root = args.output.resolve()
    root.mkdir(mode=0o700, parents=True, exist_ok=args.nested)
    if args.niri and not args.nested:
        config = root / "niri.kdl"
        config.write_text(
            'input { keyboard { xkb { layout "se"; }; }; }\nhotkey-overlay { skip-at-startup; }\n'
        )
        bus = root / "bus.conf"
        bus.write_text(
            '<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>'
        )
        command = [
            "niri",
            "-c",
            str(config),
            "--",
            "dbus-run-session",
            "--config-file=" + str(bus),
            "--",
            sys.executable,
            __file__,
            *sys.argv[1:],
            "--nested",
        ]
        with (root / "niri.log").open("w") as log:
            process = subprocess.Popen(
                command, stdout=log, stderr=subprocess.STDOUT, start_new_session=True
            )
            try:
                wait(
                    lambda: (root / "done").exists() or process.poll() is not None,
                    "nested test",
                    240,
                )
                assert (root / "result.json").exists(), (root / "niri.log").read_text()[
                    -4000:
                ]
            finally:
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=10)
        return
    if args.niri:
        assert Path(os.environ["NIRI_SOCKET"]).is_relative_to(runtime)
    else:
        assert os.environ["WAYLAND_DISPLAY"] == "wayland-agent-lab"
        os.environ.pop("NIRI_SOCKET", None)
    env = {
        **os.environ,
        "XDG_CONFIG_HOME": str(root / "config"),
        "XDG_DATA_HOME": str(root / "data"),
        "XDG_STATE_HOME": str(root / "state"),
        "XDG_CACHE_HOME": str(root / "cache"),
        "GTK_A11Y": "none",
        "ALSA_CONFIG_PATH": str(root / "alsa.conf"),
    }
    source = "koett_paste_source"
    (root / "alsa.conf").write_text(
        f'pcm.default {{ type pipewire playback_node "-1" capture_node "{source}" }}'
    )
    settings = root / "config/koett/settings.json"
    settings.parent.mkdir(parents=True)
    settings.write_text(
        json.dumps(
            {"start_at_login": False, "model_directory": str(args.model.resolve())}
        )
    )
    gui = root / "gui.py"
    gui.write_text("""import gi,json,pathlib,sys
gi.require_version('Gtk','4.0')
from gi.repository import Gtk
app=Gtk.Application(application_id='com.koett.PrivatePasteTarget')
def activate(app):
 w=Gtk.ApplicationWindow(application=app,title='Private paste entry');w.set_default_size(700,240)
 e=Gtk.Entry();e.connect('changed',lambda e:pathlib.Path(sys.argv[1]).write_text(json.dumps({'text':e.get_text()})))
 e.connect('activate',lambda e:pathlib.Path(sys.argv[1]+'.enter').write_text('enter'))
 w.set_child(e);w.present();e.grab_focus()
app.connect('activate',activate);app.run([sys.argv[0]])
""")
    terminal = root / "terminal.py"
    terminal.write_text("""import os,pathlib,sys,tty
tty.setraw(0)
p=pathlib.Path(sys.argv[1]);data=b''
while True:
 data+=os.read(0,4096);p.write_bytes(data)
""")
    processes = []
    module = None
    checks = []
    binary = str(args.binary.resolve())

    def call(*command):
        return subprocess.check_output(command, text=True, env=env)

    def control(command):
        subprocess.run([binary, command], env=env, check=True, capture_output=True)

    def kwin_script(source):
        path = root / "kwin.js"
        path.write_text(source)
        identifier = call(
            "qdbus6",
            "org.kde.KWin",
            "/Scripting",
            "loadScript",
            str(path),
            "koett-paste-test",
        ).strip()
        try:
            call(
                "qdbus6",
                "org.kde.KWin",
                "/Scripting/Script" + identifier,
                "org.kde.kwin.Script.run",
            )
        finally:
            call(
                "qdbus6",
                "org.kde.KWin",
                "/Scripting",
                "unloadScript",
                "koett-paste-test",
            )

    def focus(pid, title):
        if args.niri:

            def find_window():
                windows = json.loads(call("niri", "msg", "-j", "windows"))
                return next(
                    (window for window in windows if window["pid"] == pid), None
                )

            window = wait(find_window, title)
            call("niri", "msg", "action", "focus-window", "--id", str(window["id"]))
        else:
            kwin_script(
                "for (const w of workspace.stackingOrder) { if (w.caption === "
                + json.dumps(title)
                + ") workspace.activeWindow = w; }"
            )
        time.sleep(0.3)

    def keys(*codes):
        if args.niri:
            mapping = {29: "ctrl", 42: "shift", 30: "a", 28: "Return"}
            modifiers = [mapping[c] for c in codes[:-1]]
            command = ["wtype"]
            for modifier in modifiers:
                command += ["-M", modifier]
            command += ["-k", mapping[codes[-1]]]
            for modifier in reversed(modifiers):
                command += ["-m", modifier]
            call(*command)
        else:
            call(
                str(Path.home() / ".local/share/agent-lab/input"),
                "chord",
                *map(str, codes),
            )

    def type_text(text):
        if args.niri:
            call("wtype", "--", text)
        else:
            call(str(Path.home() / ".local/share/agent-lab/input"), "text", text)

    database = root / "data/koett/History.sqlite3"

    def rows():
        if not database.exists():
            return []
        with sqlite3.connect(database) as db:
            return db.execute(
                "SELECT text FROM transcripts ORDER BY created_at_ms"
            ).fetchall()

    def dictate(before_stop=None):
        count = len(rows())
        control("start")
        time.sleep(0.7)
        subprocess.run(
            [
                "pw-play",
                "--target=agent_lab_audio",
                str(args.model / "test_wavs/0.wav"),
            ],
            env=env,
            check=True,
        )
        time.sleep(0.3)
        if before_stop:
            before_stop()
        control("stop")
        return wait(
            lambda: (records := rows()) and len(records) > count and records[-1][0],
            "saved final transcript",
            60,
        )

    def set_shortcut(value, raw=None):
        data = json.loads(settings.read_text())
        data["paste_shortcut"] = value
        if raw:
            data["replacements"] = [
                {
                    "phrase": phrase,
                    "replacement": "Hej Åsa! Räksmörgås, öl och blåbär — 東京.",
                }
                for phrase in [raw.rstrip(".") + ".", raw.rstrip(".")]
            ]
        settings.write_text(json.dumps(data))

    try:
        module = call(
            "pactl",
            "load-module",
            "module-remap-source",
            "master=agent_lab_audio.monitor",
            "source_name=" + source,
        ).strip()
        gui_log = (root / "gtk.log").open("w")
        target = subprocess.Popen(
            [sys.executable, str(gui), str(root / "typed.json")],
            env=env,
            stdout=gui_log,
            stderr=subprocess.STDOUT,
        )
        processes.append(target)
        time.sleep(1)
        app_log = (root / "app.log").open("w")
        app = subprocess.Popen(
            [binary, "--background"], env=env, stdout=app_log, stderr=subprocess.STDOUT
        )
        processes.append(app)
        if not args.niri:
            wait(
                lambda: (root / "state/koett/remote-desktop-token").exists(),
                "private portal approval",
                90,
            )
        time.sleep(2)
        focus(target.pid, "Private paste entry")
        type_text("typed åäö ")
        raw = dictate()
        wait(
            lambda: (
                json.loads((root / "typed.json").read_text())["text"]
                == "typed åäö " + raw
            ),
            "GUI Ctrl+V consumption",
        )
        checks.append("GUI Ctrl+V consumed actual ASR text after typed Swedish prefix")
        set_shortcut("ctrl_v", raw)
        keys(29, 30)
        type_text("before ")
        text = dictate(lambda: type_text("during "))
        expected = "Hej Åsa! Räksmörgås, öl och blåbär — 東京."
        assert text == expected, (raw, text)
        wait(
            lambda: (
                json.loads((root / "typed.json").read_text())["text"]
                == "before during " + expected
            ),
            "exact GUI Unicode paste",
        )
        keys(28)
        assert (root / "typed.json.enter").read_text() == "enter"
        checks.append(
            "GUI exact Swedish/Japanese Unicode, typing during capture and Enter preserved"
        )
        term_log = (root / "kitty.log").open("w")
        term = subprocess.Popen(
            [
                "kitty",
                "--config",
                "NONE",
                "--title",
                "Private paste terminal",
                "--app-id",
                "koett-private-paste-terminal",
                sys.executable,
                str(terminal),
                str(root / "terminal.bin"),
            ],
            env=env,
            stdout=term_log,
            stderr=subprocess.STDOUT,
        )
        processes.append(term)
        time.sleep(1)
        focus(term.pid, "Private paste terminal")
        set_shortcut("ctrl_shift_v")
        type_text("typed åäö ")
        text = dictate(lambda: type_text("during "))
        assert text == expected
        wait(
            lambda: (
                (root / "terminal.bin").read_bytes()
                == ("typed åäö during " + expected).encode()
            ),
            "terminal Ctrl+Shift+V consumption",
        )
        keys(28)
        wait(
            lambda: (root / "terminal.bin").read_bytes().endswith(b"\r"),
            "terminal Enter preserved",
        )
        checks.append(
            "kitty Ctrl+Shift+V exact Unicode consumed once; typing and Enter preserved"
        )
        before = (root / "terminal.bin").read_bytes()
        set_shortcut("ctrl_v")

        def open_settings():
            control("settings")
            if args.niri:
                wait(
                    lambda: (
                        json.loads(call("niri", "msg", "-j", "focused-window")).get(
                            "title"
                        )
                        == "Settings"
                    ),
                    "Settings owns focus",
                )
            else:
                wait(
                    lambda: any(
                        w["caption"] == "Settings" and w["active"]
                        for w in json.loads(
                            call(
                                sys.executable,
                                str(Path.home() / ".local/share/agent-lab/windows.py"),
                            )
                        )
                    ),
                    "Settings owns focus",
                )
            time.sleep(0.3)

        text = dictate(open_settings)
        assert (root / "terminal.bin").read_bytes() == before
        clipboard = call("wl-paste", "--no-newline")
        assert clipboard == text
        checks.append(
            "focus changed to Settings: clipboard only, terminal unchanged, final text saved"
        )
        if args.niri:
            assert "Portal paste unavailable" in (root / "app.log").read_text()
        (root / "result.json").write_text(
            json.dumps(
                {
                    "backend": "wtype/niri"
                    if args.niri
                    else "RemoteDesktop+Clipboard/KWin",
                    "checks": checks,
                },
                indent=2,
            )
        )
        print(json.dumps(checks), flush=True)
    except Exception as error:
        (root / "failure.json").write_text(
            json.dumps({"checks": checks, "error": str(error)})
        )
        raise
    finally:
        for process in reversed(processes):
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)
        if module:
            call("pactl", "unload-module", module)
        (root / "done").write_text("done")


if __name__ == "__main__":
    main()
