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
import wave
from pathlib import Path

from test_linux_ui import wait

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--theme', choices=['light', 'dark'], required=True)
parser.add_argument('--renderer', choices=['default', 'cairo', 'gl'], default='default')

args = parser.parse_args()
runtime = f'/run/user/{os.getuid()}/agent-lab/koett-menu'
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
    assert [str(props['label']) for _, props in tray_items() if 'label' in props] == ['Ready', 'Record', 'Settings', 'History', 'Quit Koett']
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Koett") { w.frameGeometry={x:200,y:100,width:960,height:680}; workspace.activeWindow=w; }')
    menu_open()
    normal_extents = main.get_component_iface().get_extents(Atspi.CoordType.WINDOW)
    copy = menu_item('Copy last transcript')
    menu_names = [n.get_name() for n in walk(main) if n.get_role_name() == 'menu item']
    assert menu_names == ['History', 'Copy last transcript', 'Settings', 'Hide window', 'Quit Koett'], menu_names
    assert not copy.get_state_set().contains(Atspi.StateType.SENSITIVE)
    assert all(menu_item(name).get_name() for name in ['History', 'Settings', 'Hide window', 'Quit Koett'])
    capture('menu-empty-normal')
    keys('key', 1)
    wait(lambda: not find('Hide window', main), 'Escape closes menu')
    checks.append('native keyboard menu has named items; Copy last disabled before a result; Escape closes')
    keys('chord', 29, 33)
    search = next(n for n in walk(main) if n.get_role_name() == 'entry' and n.get_state_set().contains(Atspi.StateType.FOCUSED))
    assert 'EditableText' in search.get_interfaces()
    keys('chord', 29, 51)
    settings = wait(lambda: find('Settings', role='frame'), 'Ctrl+, Settings')
    wait(lambda: len(streams()) == 1, 'Settings microphone preview captures')
    click('Text', settings)
    wait(lambda: not streams(), 'Text releases microphone preview')
    click('Add replacement', settings)
    phrase = wait(lambda: find('Phrase', settings, 'text'), 'Phrase entry')
    assert phrase.get_editable_text_iface().set_text_contents('unsaved-menu-draft')
    keys('chord', 29, 51)
    assert len([n for n in walk(Atspi.get_desktop(0)) if n.get_name() == 'Settings' and n.get_role_name() == 'frame']) == 1
    assert Atspi.Text.get_text(phrase, 0, -1) == 'unsaved-menu-draft'
    tray_action('Settings')
    assert Atspi.Text.get_text(phrase, 0, -1) == 'unsaved-menu-draft'
    capture('settings-reused-draft')
    keys('chord', 56, 62)
    wait(lambda: not find('Settings', role='frame'), 'close private Settings')
    wait(lambda: not streams(), 'closing Settings releases microphone')
    menu_open()
    click_menu('Settings')
    settings = wait(lambda: find('Settings', role='frame'), 'native menu Settings')
    keys('chord', 56, 62)
    wait(lambda: not find('Settings', role='frame'), 'close menu Settings')
    click('Settings', main)
    wait(lambda: find('Settings', role='frame'), 'visible header Settings')
    keys('chord', 56, 62)
    wait(lambda: not find('Settings', role='frame'), 'close header Settings')
    checks.append('Ctrl+F focuses search; Ctrl+, and real tray Settings reuse drafts; native menu and visible header open Settings')
    menu_open()
    click_menu('History')
    wait(lambda: not find('Hide window', main), 'History closes menu')
    menu_open()
    click_menu('Hide window')
    wait(lambda: not find('Koett', role='frame'), 'Hide window')
    tray.Activate(0, 0)
    main = wait(lambda: find('Koett', role='frame'), 'tray activation opens History')
    tray_phase('Record', True)
    wait(lambda: not find('Stop', main), 'GTK receives busy state')
    checks.append('native History and Hide route correctly; actual tray activation reopens without recording')
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Koett") { w.frameGeometry={x:200,y:100,width:500,height:600}; workspace.activeWindow=w; }')
    menu_open()
    narrow_extents = main.get_component_iface().get_extents(Atspi.CoordType.WINDOW)
    capture('menu-empty-narrow')
    keys('key', 1)
    # GTK exposes window-relative component coordinates on Wayland. Position
    # only this owned window, then use compositor pointer input on the menu.
    button = find('Menu', main, 'toggle button')
    extents = button.get_component_iface().get_extents(Atspi.CoordType.WINDOW)
    assert 0 <= extents.x and extents.x + extents.width <= narrow_extents.width and 0 <= extents.y < 100, extents
    keys('move', 200 + extents.x + extents.width // 2, 100 + extents.y + extents.height // 2)
    keys('click')
    wait(lambda: find('Hide window', main), 'pointer opens native menu')
    keys('key', 1)
    checks.append('native menu fits normal and narrow widths (requested 960/500; native minimum); actual private pointer opens and Escape closes')
    # Begin with History focused so delivery is guarded, then type into an
    # owned target while real capture and ASR process the public cached sample.
    target = subprocess.Popen(['/usr/bin/python3', '-c', """
import gi
gi.require_version('Gtk','4.0')
from gi.repository import Gtk
app=Gtk.Application(application_id='com.koett.MenuTarget')
def activate(app):
 w=Gtk.ApplicationWindow(application=app,title='Menu typing target');w.set_default_size(600,300)
 e=Gtk.Entry();w.set_child(e);w.present();e.grab_focus()
app.connect('activate',activate);app.run([])
"""], stdout=(output / 'target.log').open('w'), stderr=subprocess.STDOUT)
    target_window = wait(lambda: find('Menu typing target'), 'owned target')
    wait(lambda: target_window.get_state_set().contains(Atspi.StateType.ACTIVE), 'target has focus')
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Koett") workspace.activeWindow=w;')
    tray_action('Record')
    wait(lambda: find('Stop', main), 'tray starts actual recording')
    tray_phase('Stop', True)
    menu_open()
    capture('menu-recording-narrow')
    keys('key', 1)
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Menu typing target") workspace.activeWindow=w;')
    wait(lambda: target_window.get_state_set().contains(Atspi.StateType.ACTIVE), 'target active during capture')
    keys('text', 'typing during capture')
    entry = next(n for n in walk(target_window) if 'EditableText' in n.get_interfaces())
    wait(lambda: Atspi.Text.get_text(entry, 0, -1) == 'typing during capture', 'typing not stolen')
    sample = output / 'repeated-public-sample.wav'
    with wave.open(str(model / 'test_wavs/0.wav'), 'rb') as source:
        parameters = source.getparams()
        audio = source.readframes(source.getnframes())
    with wave.open(str(sample), 'wb') as repeated:
        repeated.setparams(parameters)
        repeated.writeframes(audio * 2)
    subprocess.run(['pw-play', '--target=agent_lab_audio', str(sample)], check=True)
    assert target_window.get_state_set().contains(Atspi.StateType.ACTIVE)
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Koett") workspace.activeWindow=w;')
    menu_open()
    tray_action('Stop')
    wait(lambda: any(props.get('label') == 'Transcribing…' for _, props in tray_items()), 'real tray ASR busy')
    busy_tray = tray_phase('Transcribing…', False)
    control('start')
    tray_phase('Transcribing…', False)
    wait(lambda: not find('Stop', main), 'GTK receives busy state')
    busy_button = wait(lambda: find('Transcribing…', main, 'button'), 'busy main recording action')
    assert not busy_button.get_state_set().contains(Atspi.StateType.SENSITIVE)
    capture('menu-transcribing-narrow', settle=0)
    keys('key', 1)
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Menu typing target") workspace.activeWindow=w;')
    wait(lambda: rows(), 'private result stored', timeout=120)
    assert len(rows()) == 1, 'busy Start must not produce another capture'
    wait(lambda: not streams(), 'dictation releases microphone')
    assert target_window.get_state_set().contains(Atspi.StateType.ACTIVE)
    assert Atspi.Text.get_text(entry, 0, -1) == 'typing during capture'
    wait(lambda: find('Record', main), 'ready after real dictation')
    tray_phase('Record', True)
    checks.append('real tray Record/Stop; native menu remains usable with disabled Transcribing; extra Start rejected')
    # Keep the target active through a second complete stop/ASR cycle; the
    # first cycle deliberately foregrounded Koett to inspect its busy menu.
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Koett") workspace.activeWindow=w;')
    control('start')
    wait(lambda: find('Stop', main), 'second actual capture')
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Menu typing target") workspace.activeWindow=w;')
    subprocess.run(['pw-play', '--target=agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    control('stop')
    wait(lambda: len(rows()) == 2, 'second ASR result', timeout=120)
    assert target_window.get_state_set().contains(Atspi.StateType.ACTIVE)
    assert Atspi.Text.get_text(entry, 0, -1) == 'typing during capture'
    checks.append('second complete dictation and ASR cycle leave target active and typed contents unchanged')
    control('show')
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Koett") workspace.activeWindow=w;')
    keys('key', 15)
    menu_open()
    copy = menu_item('Copy last transcript')
    assert copy.get_state_set().contains(Atspi.StateType.SENSITIVE)
    capture('menu-completed-narrow')
    click_menu('Copy last transcript')
    wait(lambda: subprocess.check_output(['wl-paste', '--no-newline']).decode() == rows()[-1][0], 'exact native menu Copy last')
    checks.append('Copy last becomes enabled after real private dictation; clipboard exactly matches persisted ASR result')
    # Trigger a real capture failure and verify Retry on both surfaces.
    updated = json.loads(config.read_text())
    updated['microphone'] = 'invalid-private-menu-input'
    config.write_text(json.dumps(updated))
    control('start')
    wait(lambda: find('Retry', main), 'actual capture failure Retry')
    tray_phase('Retry', True)
    capture('retry-narrow')
    checks.append('real unavailable-input failure exposes Retry in tray and History')
    if args.theme == 'dark':
        menu_open()
        click_menu('Quit Koett')
    else:
        keys('chord', 29, 16)
    process.wait(timeout=10)
    assert process.returncode == 0
    checks.append(('native menu Quit' if args.theme == 'dark' else 'Ctrl+Q') + ' quits only owned test app')
    receipt = {'checks': checks, 'actual_window_sizes': {'normal': [normal_extents.width, normal_extents.height], 'narrow': [narrow_extents.width, narrow_extents.height]}, 'menu_accessible_names': menu_names, 'busy_tray': [(ident, {str(key): str(value) for key, value in properties.items()}) for ident, properties in busy_tray], 'renderer': args.renderer, 'app_cpu_affinity': 'one inherited allowed CPU to expose real ASR busy state', 'binary': str(binary), 'theme': args.theme, 'tray': 'disposable private watcher and actual dbusmenu dispatch; no rendered tray host'}
    (output / 'results.json').write_text(json.dumps(receipt, indent=2))
    print(json.dumps(receipt, indent=2))
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
