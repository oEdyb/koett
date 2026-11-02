#!/usr/bin/env python3
"""Private portal shortcut activation and live editing evidence.

Uses only its private compositor, accessibility bus, clipboard, synthetic audio,
settings and history. No app test hooks or model downloads.
"""
import argparse
import ast
import atexit
import hashlib
import json
import os
import shlex
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
parser.add_argument('--expected-gtk-library', type=Path)

args = parser.parse_args()
runtime = f'/run/user/{os.getuid()}/agent-lab/{args.lab}'
assert os.environ.get('AGENT_LAB_RUNTIME') == runtime
assert os.environ.get('XDG_RUNTIME_DIR') == runtime
assert os.environ.get('WAYLAND_DISPLAY') == 'wayland-agent-lab'
assert os.environ.get('AGENT_LAB_GENERATION')
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
binary = args.binary.resolve()
lab_data = Path(os.environ['XDG_DATA_HOME'])
assert lab_data == Path.home() / '.local/state/agent-lab' / args.lab / 'data'
desktop = lab_data / 'applications/com.olledyberg.Koett.desktop'
desktop.parent.mkdir(parents=True, exist_ok=True)
desktop.write_text(f'[Desktop Entry]\nType=Application\nName=Koett\nExec={binary}\nTerminal=false\n')
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


def shortcut_state():
    return json.loads(subprocess.check_output([str(binary), 'shortcuts-status'], text=True))


def held_input():
    tools = Path.home() / '.local/share/agent-lab'
    source = (tools / 'input.c').read_text()
    original = 'const struct timespec held = {.tv_sec = 0, .tv_nsec = 50000000};'
    assert source.count(original) == 1
    path = output / 'held-input.c'
    path.write_text(source.replace(original,
        'const struct timespec held = {.tv_sec = 2, .tv_nsec = 0};'))
    flags = shlex.split(subprocess.check_output(['pkg-config', '--cflags', '--libs',
                                               'wayland-client', 'xkbcommon'], text=True))
    helper = output / 'held-input'
    subprocess.run(['cc', '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror',
                    '-I' + str(tools / 'build'), str(path),
                    str(tools / 'build/fake-input-code.c'), *flags, '-o', str(helper)], check=True)
    return helper


def desktop_binding(ident, caption, codes):
    action = ['com.olledyberg.Koett', ident, 'Koett', caption]
    base = ['gdbus', 'call', '--session', '--dest', 'org.kde.kglobalaccel',
            '--object-path', '/kglobalaccel', '--method']
    subprocess.run([*base, 'org.kde.KGlobalAccel.doRegister', repr(action)],
                   check=True, capture_output=True)
    subprocess.run([*base, 'org.kde.KGlobalAccel.setForeignShortcut', repr(action), repr(codes)],
                   check=True, capture_output=True)


bindings = [('toggle-recording', 'Dictation', 100663328),
            ('ask', 'Ask', 201326624), ('media', 'Transcribe media', 100663380)]
# These changes target only the disposable compositor's private desktop settings.
for ident, caption, _ in bindings:
    desktop_binding(ident, caption, [])

process = target = watcher = player = None
checks = []
try:

    watcher_path = output / 'tray-service.json'
    watcher = subprocess.Popen(['/usr/bin/python3', '-c', watcher_code, str(watcher_path)],
        stdout=(output / 'watcher.log').open('w'), stderr=subprocess.STDOUT)
    wait(lambda: Path(str(watcher_path) + '.ready').exists(), 'private tray watcher')
    process = subprocess.Popen(['taskset', '--cpu-list', str(min(os.sched_getaffinity(0))), str(binary)],
        stdout=(output / 'app.log').open('w'), stderr=subprocess.STDOUT)
    wait(lambda: find('Ready'), 'real model ready', timeout=120)
    gtk_libraries = {line.split()[-1] for line in Path(f'/proc/{process.pid}/maps').read_text().splitlines()
                     if 'libgtk-4.so.' in line}
    assert len(gtk_libraries) == 1, gtk_libraries
    gtk_library = Path(gtk_libraries.pop()).resolve(strict=True)
    if args.expected_gtk_library:
        assert gtk_library == args.expected_gtk_library.resolve(strict=True)
    (output / 'runtime.json').write_text(json.dumps({
        'binary': str(binary), 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'gtk_library': str(gtk_library), 'gtk_sha256': hashlib.sha256(gtk_library.read_bytes()).hexdigest(),
    }, indent=2) + '\n')
    wait(lambda: shortcut_state().get('backend') == 'portal', 'real portal registration', timeout=30)
    unassigned = shortcut_state()
    assert not unassigned['ok'] and not unassigned['active'], unassigned
    assert set(unassigned['missing']) == {'toggle-recording', 'ask', 'media'}, unassigned
    control('settings')
    settings_window = wait(lambda: find('Settings', role='frame'), 'native Settings window')
    click('Shortcuts', settings_window)
    wait(lambda: find('Not assigned', settings_window), 'unassigned keys visible in Settings')
    capture('portal-settings-unassigned')
    for ident, caption, key in bindings:
        desktop_binding(ident, caption, [key])
    wait(lambda: len(shortcut_state().get('active', [])) == 3, 'desktop assigned bindings')
    for caption in ['Ctrl+Shift+Space', 'Ctrl+Alt+Space', 'Ctrl+Shift+T']:
        wait(lambda caption=caption: find(caption, settings_window), 'live assigned Settings key: ' + caption)
    capture('portal-settings-assigned')
    kwin(f'for (const w of workspace.windowList()) if (w.pid === {process.pid} && w.caption === "Settings") w.closeWindow();')
    wait(lambda: find('Settings', role='frame') is None, 'Settings closes before recording')
    keys('chord', 29, 42, 57)
    wait(lambda: find('Stop'), 'actual portal shortcut starts Recording')
    capture('portal-activated')
    keys('chord', 29, 42, 57)
    wait(lambda: find('Ready'), 'actual portal shortcut stops Recording')
    values = json.loads(config.read_text())
    values['recording_mode'] = 'hold'
    config.write_text(json.dumps(values))
    held = subprocess.Popen([str(held_input()), 'chord', '29', '42', '57'])
    try:
        wait(lambda: find('Stop'), 'held portal chord starts recording')
        assert held.poll() is None, 'recording begins before release'
        capture('portal-hold-recording')
    finally:
        held.wait(timeout=5)
    wait(lambda: find('Ready'), 'portal release stops hold recording')
    values['recording_mode'] = 'toggle'
    config.write_text(json.dumps(values))
    desktop_binding('toggle-recording', 'Dictation', [201326660])
    wait(lambda: shortcut_state()['triggers']['toggle-recording'] == 'Ctrl+Alt+D', 'live rebind')
    keys('chord', 29, 42, 57)
    time.sleep(0.3)
    assert find('Stop') is None, 'old binding no longer starts recording'
    keys('chord', 29, 56, 32)
    wait(lambda: find('Stop'), 'new binding starts recording')
    keys('chord', 29, 56, 32)
    wait(lambda: find('Ready'), 'new binding stops recording')
    keys('chord', 29, 56, 57)
    wait(lambda: find('Enable Ask in provider configuration first'), 'actual Ask shortcut dispatch')
    control('ask-cancel')
    keys('chord', 29, 42, 20)
    wait(lambda: find('Transcribe media', role='frame'), 'actual media shortcut dispatch')
    status = shortcut_state()
    assert status['ok'] and set(status['active']) == {'toggle-recording', 'ask', 'media'}, status
    edited = json.loads(subprocess.check_output([str(binary), 'shortcuts-edit'], text=True))
    assert edited['ok'], edited
    capture('portal-live-configure')
    (output / 'results.json').write_text(json.dumps({'unassigned': unassigned, 'status': status,
        'edit': edited, 'checks': ['unassigned IDs reported unavailable',
        'private desktop key assignment reflected live', 'actual portal Recording activation and stop',
        'actual Ask and media shortcut dispatch', 'hold press starts and release stops',
        'live rebind disables old key and enables new key', 'live ConfigureShortcuts call']}, indent=2))

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
