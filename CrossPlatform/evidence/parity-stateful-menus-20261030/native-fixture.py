"""Owned koett-stateful-menus acceptance; temporary private state and audio only."""

import argparse
import ast
import atexit
import hashlib
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('theme', choices=['light', 'dark'], nargs='?', default='light')
parser.add_argument('--lab', default='koett-stateful-menus')
parser.add_argument('--binary', type=Path, default=Path('/home/olle/dev/koett/CrossPlatform/target/release/koett'))
parser.add_argument('--output', type=Path)
parser.add_argument('--expected-gtk-library', type=Path)
args = parser.parse_args()
theme = args.theme
assert os.environ['AGENT_LAB_RUNTIME'] == f'/run/user/{os.getuid()}/agent-lab/{args.lab}'
assert os.environ['XDG_RUNTIME_DIR'] == os.environ['AGENT_LAB_RUNTIME']
root = args.output or Path(os.environ['XDG_STATE_HOME']) / 'menu-fixtures'
root.mkdir(parents=True, exist_ok=False)
for kind in ["CONFIG", "DATA", "CACHE", "STATE"]:
    os.environ["XDG_" + kind + "_HOME"] = str(root / kind.lower())
config = root / "config/koett/settings.json"
config.parent.mkdir(parents=True, exist_ok=True)
model = (
    Path.home()
    / ".local/share/koett/models/sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8"
)
assert model.is_dir()
manifest = json.loads((Path(__file__).resolve().parents[2] / 'model-manifests/parakeet-tdt-ctc-110m-int8.json').read_text())
private_model = root / 'models' / manifest['model_id']
private_model.mkdir(parents=True)
for item in manifest['files']:
    destination = private_model / item['path']
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(model / item['path'], destination)
    assert hashlib.sha256(destination.read_bytes()).hexdigest() == item['sha256']
model = private_model
os.environ['KOETT_MODEL_DIR'] = str(model)
config.write_text(
    json.dumps(
        {"model_directory": str(model), "start_at_login": False, "pause_media": False}
    )
)
alsa = root / "alsa.conf"
alsa.write_text(
    '</usr/share/alsa/alsa.conf>\npcm.!default { type pulse }\npcm.lab { type pulse hint { show on description "Private lab microphone" ioid "Input" } }\n'
)
os.environ["ALSA_CONFIG_PATH"] = str(alsa)
os.environ["PULSE_SOURCE"] = "agent_lab_audio.monitor"
os.environ.update(GTK_A11Y="atspi", GTK_THEME="Adwaita:" + theme, GSK_RENDERER="cairo")
address = ast.literal_eval(
    subprocess.check_output(
        [
            "gdbus",
            "call",
            "--session",
            "--dest",
            "org.a11y.Bus",
            "--object-path",
            "/org/a11y/bus",
            "--method",
            "org.a11y.Bus.GetAddress",
        ],
        text=True,
    ).strip()
)[0]
os.environ["AT_SPI_BUS_ADDRESS"] = address
registry = subprocess.Popen(
    ["/usr/lib/at-spi2-registryd"],
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
)
atexit.register(registry.terminate)
binary = str(args.binary.resolve())
watcher_code = "\nimport json,sys\nfrom pathlib import Path\nimport dbus, dbus.service, dbus.mainloop.glib\nfrom gi.repository import GLib\ndbus.mainloop.glib.DBusGMainLoop(set_as_default=True)\nbus=dbus.SessionBus()\nname=dbus.service.BusName('org.kde.StatusNotifierWatcher',bus,do_not_queue=True)\nclass Watcher(dbus.service.Object):\n @dbus.service.method('org.kde.StatusNotifierWatcher',in_signature='s',out_signature='')\n def RegisterStatusNotifierItem(self,service):\n  Path(sys.argv[1]).write_text(json.dumps(str(service)))\n @dbus.service.method('org.freedesktop.DBus.Properties',in_signature='ss',out_signature='v')\n def Get(self,interface,prop):\n  return {'IsStatusNotifierHostRegistered':dbus.Boolean(True),'ProtocolVersion':dbus.Int32(0),'RegisteredStatusNotifierItems':dbus.Array([],signature='s')}[prop]\n @dbus.service.method('org.freedesktop.DBus.Properties',in_signature='s',out_signature='a{sv}')\n def GetAll(self,interface):\n  return {'IsStatusNotifierHostRegistered':dbus.Boolean(True),'ProtocolVersion':dbus.Int32(0),'RegisteredStatusNotifierItems':dbus.Array([],signature='s')}\nwatcher=Watcher(bus,'/StatusNotifierWatcher')\nPath(sys.argv[1]+'.ready').touch()\nGLib.MainLoop().run()\n"
watcher = subprocess.Popen(
    ["/usr/bin/python3", "-c", watcher_code, str(root / "watcher.json")]
)
atexit.register(watcher.terminate)
time.sleep(0.3)
log = (root / "app.log").open("w")
app = subprocess.Popen([binary], stdout=log, stderr=log)
atexit.register(lambda: app.terminate() if app.poll() is None else None)


def call(*args):
    p = subprocess.run([binary, *args], check=False, capture_output=True, text=True)
    value = json.loads(p.stdout)
    assert (p.returncode == 0) == value["ok"], (p.returncode, value, p.stderr)
    return value


def wait():
    for _ in range(100):
        try:
            s = call("menu-status")
            if not s["busy"]:
                return s
        except (json.JSONDecodeError, KeyError) as error:
            print(f"Waiting for controller: {error}", flush=True)
        assert app.poll() is None
        time.sleep(0.2)
    raise AssertionError("not ready")


try:
    initial = wait()
    print("Ready with active cached engine", initial['active_engine'], flush=True)
    gtk_paths = {line.split()[-1] for line in Path(f'/proc/{app.pid}/maps').read_text().splitlines() if 'libgtk-4.so.' in line}
    assert len(gtk_paths) == 1, gtk_paths
    gtk_library = Path(gtk_paths.pop()).resolve()
    if args.expected_gtk_library:
        assert gtk_library == args.expected_gtk_library.resolve()
    assert not call('menu-model', 'invented')['ok']
    assert call('menu-model', 'compact')['status'] == 'change_requested'
    switched = wait()
    assert switched['desired_model'] == 'compact'
    assert json.loads(config.read_text())['model_directory'] is None
    for field, value in [('style', 'formal'), ('structure', 'lists'), ('context', 'email')]:
        assert call('menu-format', field, value)['status'] == 'change_requested'
        changed = wait()
        assert changed['formatting'][field] == value
        assert changed['formatting']['enabled'] is False
    assert not call('menu-format', 'style', 'invented')['ok']
    assert call('menu-status')['formatting']['style'] == 'formal'
    print('Direct cached-model CLI selection and complete Raw formatting field patches PASS', flush=True)
    assert call("menu-mode", "hold")["ok"]
    assert call("menu-status")["recording_mode"] == "hold"
    assert json.loads(config.read_text())["pause_media"] is False
    assert call("menu-login", "on")["ok"]
    assert (root / "config/autostart/com.olledyberg.Koett.desktop").exists()
    assert call("menu-login", "off")["ok"]
    assert call("menu-microphone", "default")["ok"]
    assert not call("menu-microphone", "missing-test-device")["ok"]
    assert call("menu-status")["microphone"] is None
    saved = config.read_text()
    config.unlink()
    config.mkdir()
    assert not call("menu-mode", "toggle")["ok"]
    config.rmdir()
    config.write_text(saved)
    assert call("menu-status")["recording_mode"] == "hold"
    print("CLI persistence, failure, unrelated-field preservation PASS", flush=True)
    subprocess.run(
        [
            "/usr/bin/python3",
            str(Path.home() / ".local/share/agent-lab/screenshot.py"),
            f"{root}/current-{theme}-window.png",
        ],
        check=True,
    )
    print("ready for native menu", flush=True)
    tool = str(Path.home() / ".local/share/agent-lab/input")
    subprocess.run([tool, "move", "1350", "227"], check=True)
    subprocess.run([tool, "click", "272"], check=True)
    time.sleep(0.3)
    subprocess.run([tool, "key", "108", "108", "108", "106"], check=True)
    time.sleep(0.4)
    subprocess.run([tool, "key", "108", "108", "108", "108", "106"], check=True)
    time.sleep(0.4)
    subprocess.run(
        [
            "/usr/bin/python3",
            str(Path.home() / ".local/share/agent-lab/screenshot.py"),
            f"{root}/current-{theme}-mode.png",
        ],
        check=True,
    )
    subprocess.run([tool, "move", "1280", "307"], check=True)
    subprocess.run([tool, "click", "272"], check=True)
    time.sleep(0.5)
    assert call("menu-status")["recording_mode"] == "toggle"
    print(
        "Native keyboard navigation and pointer selection persisted Toggle PASS",
        flush=True,
    )
    from gi.repository import Gio, GLib

    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    names = bus.call_sync(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "ListNames",
        None,
        None,
        0,
        3000,
        None,
    ).unpack()[0]
    dest = next(
        n
        for n in names
        if n.startswith("org.kde.StatusNotifierItem-" + str(app.pid) + "-")
    )
    menu = bus.call_sync(
        dest,
        "/StatusNotifierItem",
        "org.freedesktop.DBus.Properties",
        "Get",
        GLib.Variant("(ss)", ("org.kde.StatusNotifierItem", "Menu")),
        None,
        0,
        3000,
        None,
    ).unpack()[0]

    def layout():
        return bus.call_sync(
            dest,
            menu,
            "com.canonical.dbusmenu",
            "GetLayout",
            GLib.Variant("(iias)", (0, -1, [])),
            None,
            0,
            3000,
            None,
        ).unpack()[1]

    def rows(n):
        yield n
        for child in n[2]:
            yield from rows(child)

    entries = list(rows(layout()))
    hold = next(n for n in entries if n[1].get("label") == "Hold")
    assert hold[1]["toggle-state"] == 0
    call("menu-mode", "hold")
    time.sleep(0.7)
    assert (
        next(n for n in rows(layout()) if n[1].get("label") == "Hold")[1][
            "toggle-state"
        ]
        == 1
    )
    toggle = next(n for n in entries if n[1].get("label") == "Toggle")
    bus.call_sync(
        dest,
        menu,
        "com.canonical.dbusmenu",
        "Event",
        GLib.Variant("(isvu)", (toggle[0], "clicked", GLib.Variant("s", ""), 0)),
        None,
        0,
        3000,
        None,
    )
    time.sleep(0.7)
    assert call("menu-status")["recording_mode"] == "toggle"
    assert (
        next(n for n in rows(layout()) if n[1].get("label") == "Toggle")[1][
            "toggle-state"
        ]
        == 1
    )
    print(
        "Real tray D-Bus checked Hold -> Toggle persistence PASS; no tray rendering claim",
        flush=True,
    )
    devices = call("menu-status")["microphones"]["devices"]
    device = next(d["id"] for d in devices if d["label"] == "Private lab microphone")
    assert call("menu-microphone", device)["ok"]
    capture_offset = (root / "app.log").stat().st_size
    subprocess.run([binary, "start"], check=True)
    assert not call('menu-model', 'compact')['ok']
    blocked = call("menu-mode", "hold")
    assert not blocked["ok"], blocked
    assert call("menu-status")["microphone"] == device
    time.sleep(0.5)
    subprocess.run([binary, "stop"], check=True)
    wait()
    opened = (root / "app.log").read_bytes()[capture_offset:].decode()
    assert "microphone=Private lab microphone sample_rate=" in opened, opened
    assert call("menu-microphone", "default")["ok"]
    print(
        "Selected private CPAL input next capture; busy preference rejection PASS",
        flush=True,
    )
    assert call("menu-format", "raw")["ok"]
    wait()
    assert call("menu-status")["formatting"]["enabled"] is False
    assert call("menu-open", "about")["ok"]
    time.sleep(0.5)
    subprocess.run(
        [
            "/usr/bin/python3",
            str(Path.home() / ".local/share/agent-lab/screenshot.py"),
            f"{root}/current-{theme}-about.png",
        ],
        check=True,
    )
    subprocess.run([tool, "key", "1"], check=True)
    time.sleep(0.3)
    geometry = root / "narrow.js"
    geometry.write_text(
        'for (const w of workspace.stackingOrder) if (w.caption === "Koett") { w.frameGeometry={x:200,y:100,width:500,height:680}; workspace.activeWindow=w; }'
    )
    ident = subprocess.check_output(
        [
            "qdbus6",
            "org.kde.KWin",
            "/Scripting",
            "loadScript",
            str(geometry),
            "stateful-narrow",
        ],
        text=True,
    ).strip()
    try:
        subprocess.run(
            [
                "qdbus6",
                "org.kde.KWin",
                "/Scripting/Script" + ident,
                "org.kde.kwin.Script.run",
            ],
            check=True,
            capture_output=True,
        )
    finally:
        subprocess.run(
            ["qdbus6", "org.kde.KWin", "/Scripting", "unloadScript", "stateful-narrow"],
            check=True,
            capture_output=True,
        )
    time.sleep(0.5)
    subprocess.run([tool, "move", "640", "127"], check=True)
    subprocess.run([tool, "click", "272"], check=True)
    time.sleep(0.3)
    subprocess.run(
        [
            "/usr/bin/python3",
            str(Path.home() / ".local/share/agent-lab/screenshot.py"),
            f"{root}/current-{theme}-narrow.png",
        ],
        check=True,
    )

    (root / 'results.json').write_text(json.dumps({
        'ok': True, 'binary_sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
        'gtk_library': str(gtk_library), 'gtk_sha256': hashlib.sha256(gtk_library.read_bytes()).hexdigest(),
        'final_status': call('menu-status'),
        'checks': ['direct cached model selection', 'all formatting field CLI patches',
                   'preference persistence and save failure', 'native keyboard and pointer menu selection',
                   'real tray checked-state and action', 'private microphone next capture',
                   'busy model and preference rejection', 'About and narrow layout'],
    }, indent=2) + '\n')

finally:
    app.terminate()
    app.wait(timeout=10)
