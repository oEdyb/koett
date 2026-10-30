#!/usr/bin/env python3
"""Native menu evidence; caller owns agent-lab koett-menu.

Uses only its private compositor, accessibility bus, clipboard, synthetic audio,
settings and history. No app test hooks or model downloads.
"""
import argparse
import ast
import atexit
import json
import os
import sqlite3
import subprocess
import time
from pathlib import Path

from test_linux_ui import wait

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--lab', default='koett-menu')
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--theme', choices=['light', 'dark'], required=True)
parser.add_argument('--renderer', choices=['default', 'cairo', 'gl'], default='default')

args = parser.parse_args()
runtime = f'/run/user/{os.getuid()}/agent-lab/{args.lab}'
assert os.environ.get('AGENT_LAB_RUNTIME') == runtime
assert os.environ.get('XDG_RUNTIME_DIR') == runtime
assert os.environ.get('WAYLAND_DISPLAY') == 'wayland-agent-lab'
assert os.environ.get('AGENT_LAB_GENERATION')
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
binary = args.binary.resolve()
model = Path.home() / '.local/share/koett/models/sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8'
assert model.is_dir()
os.environ.pop('NIRI_SOCKET', None)
os.environ.pop('AT_SPI_BUS_ADDRESS', None)
for kind in ['CONFIG', 'DATA', 'CACHE', 'STATE']:
    os.environ[f'XDG_{kind}_HOME'] = str(output / kind.lower())
os.environ.update(GTK_A11Y='atspi', GTK_THEME='Adwaita:' + args.theme, GTK_USE_PORTAL='0')
if args.renderer != 'default':
    os.environ['GSK_RENDERER'] = args.renderer
config = output / 'config/koett/settings.json'
config.parent.mkdir(parents=True)
config.write_text(json.dumps({'model_directory': str(model), 'start_at_login': False}))
(output / 'config/user-dirs.dirs').write_text(f'XDG_DOCUMENTS_DIR="{output}"\n')
module = subprocess.check_output(['pactl', 'load-module', 'module-remap-source',
    'master=agent_lab_audio.monitor', 'source_name=menu_mic'], text=True).strip()
atexit.register(lambda: subprocess.run(['pactl', 'unload-module', module], check=True))
alsa = output / 'alsa.conf'
alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "menu_mic" }\nctl.!default { type pulse }\npcm.menu_input { type pulse device "menu_mic" hint { show on description "Private menu input" ioid "Input" } }\n')
os.environ.update(ALSA_CONFIG_PATH=str(alsa), PULSE_SOURCE='menu_mic')
address = ast.literal_eval(subprocess.check_output(['gdbus', 'call', '--session', '--dest', 'org.a11y.Bus', '--object-path', '/org/a11y/bus', '--method', 'org.a11y.Bus.GetAddress'], text=True).strip())[0]
os.environ['AT_SPI_BUS_ADDRESS'] = address
registry = subprocess.Popen(['/usr/lib/at-spi2-registryd'], stdout=(output / 'accessibility.log').open('w'), stderr=subprocess.STDOUT)
def stop_registry():
    if registry.poll() is None:
        registry.terminate()
        registry.wait(timeout=5)


atexit.register(stop_registry)
subprocess.run(['gdbus', 'call', '--session', '--dest', 'org.a11y.Bus', '--object-path', '/org/a11y/bus', '--method', 'org.freedesktop.DBus.Properties.Set', 'org.a11y.Status', 'IsEnabled', '<true>'], check=True, stdout=subprocess.DEVNULL)
import gi

gi.require_version('Atspi', '2.0')
from gi.repository import Atspi


def walk(node):
    yield node
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child:
            yield from walk(child)


def find(name, root=None, role=None):
    for node in walk(root or Atspi.get_desktop(0)):
        node.clear_cache()
        if node.get_name() == name and (role is None or node.get_role_name() == role) and node.get_state_set().contains(Atspi.StateType.SHOWING):
            return node
    return None


def click(name, root=None):
    if name in ['Audio', 'Shortcuts', 'Text', 'General']:
        combo = wait(lambda: find('Settings section', root, 'combo box'), 'Settings selector')
        kwin(f'for (const w of workspace.windowList()) if (w.pid === {process.pid} && w.caption === "Settings") {{ w.frameGeometry={{x:400,y:140,width:540,height:460}}; workspace.activeWindow=w; }}')
        rect = combo.get_component_iface().get_extents(Atspi.CoordType.WINDOW)
        input_tool = str(Path.home() / '.local/share/agent-lab/input')
        subprocess.run([input_tool, 'move', str(400 + rect.x + 50), str(140 + rect.y + rect.height // 2)], check=True)
        subprocess.run([input_tool, 'click'], check=True)
        keys('key', 102)
        for _ in range(['Audio', 'Shortcuts', 'Text', 'General'].index(name)):
            keys('key', 108)
        keys('key', 28)
        time.sleep(0.4)
        return combo
    node = wait(lambda: find(name, root), name)
    assert node.get_action_iface().do_action(0), name
    time.sleep(0.2)
    return node


def control(command):
    subprocess.run([str(binary), command], check=True, capture_output=True)


def capture(name, settle=0.3):
    time.sleep(settle)
    subprocess.run(['/usr/bin/python3', str(Path.home() / '.local/share/agent-lab/screenshot.py'), str(output / (name + '.png'))], check=True, stdout=subprocess.DEVNULL)
    print('Captured ' + name, flush=True)


def streams():
    clients = json.loads(subprocess.check_output(['pactl', '-f', 'json', 'list', 'clients']))
    owned = {str(c['index']) for c in clients if str(c.get('properties', {}).get('application.process.id')) == str(process.pid)}
    return [s for s in json.loads(subprocess.check_output(['pactl', '-f', 'json', 'list', 'source-outputs'])) if str(s.get('client')) in owned]


def keys(command, *codes):
    subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), command, *map(str, codes)], check=True)


def kwin(source):
    path = output / 'window.js'
    path.write_text(source)
    ident = subprocess.check_output(['qdbus6', 'org.kde.KWin', '/Scripting', 'loadScript', str(path), 'menu-test'], text=True).strip()
    try:
        subprocess.run(['qdbus6', 'org.kde.KWin', '/Scripting/Script' + ident, 'org.kde.kwin.Script.run'], check=True, capture_output=True)
    finally:
        subprocess.run(['qdbus6', 'org.kde.KWin', '/Scripting', 'unloadScript', 'menu-test'], check=True, capture_output=True)
    time.sleep(0.3)


watcher_code = r"""
import json,sys
from pathlib import Path
import dbus, dbus.service, dbus.mainloop.glib
from gi.repository import GLib
dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
bus=dbus.SessionBus()
name=dbus.service.BusName('org.kde.StatusNotifierWatcher',bus,do_not_queue=True)
class Watcher(dbus.service.Object):
 @dbus.service.method('org.kde.StatusNotifierWatcher',in_signature='s',out_signature='')
 def RegisterStatusNotifierItem(self,service):
  Path(sys.argv[1]).write_text(json.dumps(str(service)))
 @dbus.service.method('org.freedesktop.DBus.Properties',in_signature='ss',out_signature='v')
 def Get(self,interface,prop):
  return {'IsStatusNotifierHostRegistered':dbus.Boolean(True),'ProtocolVersion':dbus.Int32(0),'RegisteredStatusNotifierItems':dbus.Array([],signature='s')}[prop]
 @dbus.service.method('org.freedesktop.DBus.Properties',in_signature='s',out_signature='a{sv}')
 def GetAll(self,interface):
  return {'IsStatusNotifierHostRegistered':dbus.Boolean(True),'ProtocolVersion':dbus.Int32(0),'RegisteredStatusNotifierItems':dbus.Array([],signature='s')}
watcher=Watcher(bus,'/StatusNotifierWatcher')
Path(sys.argv[1]+'.ready').touch()
GLib.MainLoop().run()
"""


def rows():
    path = output / 'data/koett/History.sqlite3'
    if not path.exists():
        return []
    with sqlite3.connect(path) as connection:
        return connection.execute('SELECT text FROM transcripts ORDER BY created_at_ms').fetchall()


def menu_open():
    button = wait(lambda: find('Menu', main, 'toggle button'), 'Menu button')
    keys('chord', 29, 33)
    for _ in range(20):
        keys('chord', 42, 15)
        if button.get_state_set().contains(Atspi.StateType.FOCUSED):
            break
    assert button.get_state_set().contains(Atspi.StateType.FOCUSED), 'Menu reachable by keyboard'
    keys('key', 57)
    wait(lambda: find('Hide window', main), 'native menu opened with keyboard')


def menu_item(label):
    return wait(lambda: find(label, main, 'menu item'), label)


def click_menu(label):
    node = menu_item(label)
    assert node.get_action_iface().do_action(0), label
    time.sleep(0.2)


def tray_items():
    _, layout = tray_menu.GetLayout(0, -1, [])
    return [(int(child[0]), dict(child[1])) for child in layout[2]]


def tray_action(label):
    ident, properties = next(item for item in tray_items() if item[1].get('label') == label)
    assert properties.get('enabled', True), (label, properties)
    tray_menu.Event(ident, 'clicked', dbus.String(''), dbus.UInt32(0))


def tray_phase(label, enabled):
    items = tray_items()
    assert all(not str(props.get('label', '')).startswith('@') for _, props in items)
    actions = [props for _, props in items if props.get('type') != 'separator'][1:]
    assert actions[0]['label'] == label, actions
    assert bool(actions[0].get('enabled', True)) == enabled, actions[0]
    return items


process = target = watcher = player = None
checks = []
try:
    import dbus

    watcher_path = output / 'tray-service.json'
    watcher = subprocess.Popen(['/usr/bin/python3', '-c', watcher_code, str(watcher_path)],
        stdout=(output / 'watcher.log').open('w'), stderr=subprocess.STDOUT)
    wait(lambda: Path(str(watcher_path) + '.ready').exists(), 'private tray watcher')
    process = subprocess.Popen(['taskset', '--cpu-list', str(min(os.sched_getaffinity(0))), str(binary)],
        stdout=(output / 'app.log').open('w'), stderr=subprocess.STDOUT)
    wait(lambda: find('Ready'), 'real model ready', timeout=120)
    main = wait(lambda: find('Koett', role='frame'), 'History window')
    service = wait(lambda: json.loads(watcher_path.read_text()) if watcher_path.exists() else None, 'real tray registration')
    bus = dbus.SessionBus()
    tray = dbus.Interface(bus.get_object(service, '/StatusNotifierItem'), 'org.kde.StatusNotifierItem')
    tray_menu = dbus.Interface(bus.get_object(service, '/MenuBar'), 'com.canonical.dbusmenu')
    tray_phase('Record', True)
    labels = [str(props['label']) for _, props in tray_items() if 'label' in props]
    assert 'Copy last transcript' in labels and 'Failed recordings…' in labels
    tray_action('Settings')
    wait(lambda: find('Settings', role='frame'), 'actual tray DBus Settings')
    capture('settings-tray')
    keys('chord', 56, 62)
    wait(lambda: not find('Settings', role='frame'), 'Settings closed')
    kwin('for (const w of workspace.windowList()) if (w.caption === "Koett") { w.frameGeometry={x:200,y:100,width:480,height:560}; workspace.activeWindow=w; }')
    menu_open()
    for label in ['Ask', 'Transcripts', 'Recovery', 'Preferences']:
        assert find(label, main), label
    capture('menu-narrow')
    keys('key', 1)
    control('start')
    quit_id = next(ident for ident, props in tray_items() if props.get('label') == 'Quit Koett')
    tray_menu.Event(quit_id, 'clicked', dbus.String(''), dbus.UInt32(0))
    wait(lambda: find('Stop', main), 'queued Start survives ordinary Quit')
    assert process.poll() is None
    capture('queued-start-quit')
    control('stop')
    wait(lambda: find('Ready'), 'capture completed before Ask')
    control('ask-start')
    time.sleep(1)
    status = json.loads(subprocess.check_output([str(binary), 'ask-status'], text=True))
    assert status.get('phase') == 'Error', status
    control('quit')
    process.wait(timeout=10)
    assert process.returncode == 0
    (output / 'results.json').write_text(json.dumps({'tray_labels': labels, 'checks': ['actual DBus Settings', 'native narrow submenu names', 'queued Start survives ordinary Quit', 'disabled Ask followed by Quit'], 'binary': str(binary)} ,indent=2))
except Exception:
    capture('failure')
    (output / 'accessibility.json').write_text(json.dumps([{'name': n.get_name(), 'role': n.get_role_name(), 'states': str(n.get_state_set().get_states())} for n in walk(Atspi.get_desktop(0))], indent=2))
    raise
finally:
    for child in [player, target, process, watcher]:
        if child and child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)
