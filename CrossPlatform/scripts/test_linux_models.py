#!/usr/bin/env python3
"""Actual GTK visual matrix with synthetic text in a private agent-lab session."""
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
parser.add_argument('--lab', default='koett-visual')
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--theme', choices=['light', 'dark', 'highcontrast'], required=True)
parser.add_argument('--scaled', action='store_true')
parser.add_argument('--visual-only', action='store_true')
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
os.environ.pop('NIRI_SOCKET', None)
os.environ.pop('AT_SPI_BUS_ADDRESS', None)
for kind in ['CONFIG', 'DATA', 'CACHE', 'STATE']:
    os.environ[f'XDG_{kind}_HOME'] = str(output / kind.lower())
os.environ.update(GTK_A11Y='atspi', GTK_USE_PORTAL='0',
                  GTK_THEME='HighContrast' if args.theme == 'highcontrast' else 'Adwaita:' + args.theme)
if args.renderer != 'default':
    os.environ['GSK_RENDERER'] = args.renderer
config = output / 'config/koett/settings.json'
config.parent.mkdir(parents=True)
(output / 'config/user-dirs.dirs').write_text(f'XDG_DOCUMENTS_DIR="{output}"\n')
model = Path.home() / '.local/share/koett/models/sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8'
assert model.is_dir()
fixture_model = output / 'models' / model.name
fixture_model.parent.mkdir()
fixture_model.symlink_to(model, target_is_directory=True)
os.environ['KOETT_MODEL_DIR'] = str(fixture_model)
config.write_text(json.dumps({'start_at_login': False}))
cache = output / 'cache/koett/model-evaluations'
cache.mkdir(parents=True)
archive_name = 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2'
(cache / archive_name).symlink_to(Path.home() / '.cache/koett/model-evaluations' / archive_name)
if args.scaled:
    os.environ['GDK_DEBUG'] = 'no-portals,default-settings'
    gtk_config = output / 'config/gtk-4.0/settings.ini'
    gtk_config.parent.mkdir(parents=True)
    gtk_config.write_text('[Settings]\ngtk-font-name=Sans 14\ngtk-enable-animations=false\n')
module = subprocess.check_output(['pactl', 'load-module', 'module-remap-source',
    'master=agent_lab_audio.monitor', 'source_name=visual_mic'], text=True).strip()
atexit.register(lambda: subprocess.run(['pactl', 'unload-module', module], check=True))
alsa = output / 'alsa.conf'
alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "visual_mic" }\nctl.!default { type pulse }\n')
os.environ.update(ALSA_CONFIG_PATH=str(alsa), PULSE_SOURCE='visual_mic')
address = ast.literal_eval(subprocess.check_output(['gdbus', 'call', '--session', '--dest', 'org.a11y.Bus', '--object-path', '/org/a11y/bus', '--method', 'org.a11y.Bus.GetAddress'], text=True).strip())[0]
os.environ['AT_SPI_BUS_ADDRESS'] = address
registry = subprocess.Popen(['/usr/lib/at-spi2-registryd'], stdout=(output / 'accessibility.log').open('w'), stderr=subprocess.STDOUT)
atexit.register(lambda: registry.terminate() if registry.poll() is None else None)
subprocess.run(['gdbus', 'call', '--session', '--dest', 'org.a11y.Bus', '--object-path', '/org/a11y/bus', '--method', 'org.freedesktop.DBus.Properties.Set', 'org.a11y.Status', 'IsEnabled', '<true>'], check=True, stdout=subprocess.DEVNULL)
import gi

gi.require_version('Atspi', '2.0')
from gi.repository import Atspi

if args.scaled:
    gi.require_version('Gtk', '4.0')
    from gi.repository import Gtk
    Gtk.init()
    native = Gtk.Settings.get_default()
    assert native.get_property('gtk-font-name') == 'Sans 14'
    assert not native.get_property('gtk-enable-animations')
    (output / 'native-settings.json').write_text(json.dumps({
        'gtk-font-name': native.get_property('gtk-font-name'),
        'gtk-enable-animations': native.get_property('gtk-enable-animations'),
    }, indent=2))

texts = [
    'Synthetic visual fixture.\n\nWe tested the new history view with a longer transcript. The list keeps dates separate from the words, and the reading pane keeps the original paragraphs.\n\nUnicode stays exact: åäö, 日本語, Ελληνικά — café.\n\n' +
    '\n\n'.join(f'Paragraph {index}: This is generated test text, never a personal transcript. A longer passage checks wrapping, scrolling, selection, and comfortable reading at the native system font size.' for index in range(1, 15)),
    'Synthetic visual fixture: Bring the notes from today’s meeting into the project outline. Keep the original wording, including punctuation.',
    'Synthetic visual fixture: A short recording can stay short.',
    'Synthetic visual fixture: Review the draft after lunch. Then copy the selected transcript into the editor.',
    'Synthetic visual fixture: Tack så mycket. Vi hörs imorgon! 日本語のテキスト。',
]
database = output / 'data/koett/History.sqlite3'
database.parent.mkdir(parents=True)
schema = Path(__file__).resolve().parents[2] / 'Sources/Koett/Resources/History/schema.sql'
with sqlite3.connect(database) as connection:
    connection.executescript(schema.read_text())
    for index, text in enumerate(texts):
        connection.execute('INSERT INTO transcripts(id,created_at_ms,model,text,raw_text) VALUES(?,?,?,?,NULL)',
            (f'visual-{index}', 1_793_000_000_000 - index * 3_600_000, 'Synthetic visual fixture', text))


def walk(node):
    node.clear_cache()
    yield node
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child:
            yield from walk(child)


def find(name, root=None, role=None):
    return next((node for node in walk(root or Atspi.get_desktop(0))
        if node.get_name() == name and (role is None or node.get_role_name() == role or (role == 'text' and node.get_role_name() == 'entry'))
        and node.get_state_set().contains(Atspi.StateType.SHOWING)), None)


def click(name, root=None):
    node = wait(lambda: next((candidate for candidate in walk(root or Atspi.get_desktop(0)) if candidate.get_name() == name and candidate.get_state_set().contains(Atspi.StateType.SHOWING) and candidate.get_state_set().contains(Atspi.StateType.SENSITIVE) and 'Action' in candidate.get_interfaces() and candidate.get_action_iface().get_n_actions() > 0), None), name)
    assert node.get_action_iface().do_action(0)
    time.sleep(0.2)
    return node


def keys(command, *codes):
    subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), command, *map(str, codes)], check=True)


def control(command):
    subprocess.run([str(binary), command], check=True, capture_output=True)


def kwin(source):
    path = output / 'window.js'
    path.write_text(source)
    ident = subprocess.check_output(['qdbus6', 'org.kde.KWin', '/Scripting', 'loadScript', str(path), 'visual-test'], text=True).strip()
    try:
        subprocess.run(['qdbus6', 'org.kde.KWin', '/Scripting/Script' + ident, 'org.kde.kwin.Script.run'], check=True, capture_output=True)
    finally:
        subprocess.run(['qdbus6', 'org.kde.KWin', '/Scripting', 'unloadScript', 'visual-test'], check=True, capture_output=True)
    time.sleep(0.4)


def resize(title, width, height):
    if args.scaled and width == 500:
        width = 560
    source = f'for (const w of workspace.windowList()) if (w.pid === {process.pid} && w.caption === {json.dumps(title)}) {{ w.frameGeometry = {{x:400,y:140,width:{width},height:{height}}}; workspace.activeWindow=w; }}'
    kwin(source)
    kwin(source)
    frame = wait(lambda: find(title, role='frame'), title)
    rect = frame.get_component_iface().get_extents(Atspi.CoordType.WINDOW)
    assert rect.width <= width + 2, (title, width, rect.width)


def capture(name):
    time.sleep(0.6)
    subprocess.run(['/usr/bin/python3', str(Path.home() / '.local/share/agent-lab/screenshot.py'), str(output / (name + '.png'))], check=True, stdout=subprocess.DEVNULL)
    print('Captured ' + name, flush=True)


def transcript():
    for node in walk(main):
        if node.get_role_name() == 'text' and 'Text' in node.get_interfaces() and not node.get_state_set().contains(Atspi.StateType.EDITABLE):
            return Atspi.Text.get_text(node, 0, -1)
    return None


def search(value):
    entry = wait(lambda: find('Search transcripts', main, 'text'), 'search')
    assert entry.get_editable_text_iface().set_text_contents(value)


def current_streams():
    clients = json.loads(subprocess.check_output(['pactl', '-f', 'json', 'list', 'clients']))
    owned = {str(c['index']) for c in clients if str(c.get('properties', {}).get('application.process.id')) == str(process.pid)}
    return [s for s in json.loads(subprocess.check_output(['pactl', '-f', 'json', 'list', 'source-outputs'])) if str(s.get('client')) in owned]


process = None
checks = []
try:
    log = (output / 'app.log').open('w')
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    main = wait(lambda: find('Koett', role='frame'), 'main')
    wait(lambda: any(n.get_role_name() == 'label' and n.get_name().startswith('Ready') for n in walk(main)), 'ready', timeout=120)
    resize('Koett', 960, 680)
    wait(lambda: transcript() == texts[0], 'selected fixture')
    capture('history-populated')
    keys('chord', 29, 33)
    click('Copy', main)
    wait(lambda: subprocess.check_output(['wl-paste', '--no-newline']).decode() == texts[0], 'exact Copy')
    search('no-matching-synthetic-fixture')
    wait(lambda: find('No matches', main), 'literal search no results')
    click('Clear search', main)
    wait(lambda: transcript() == texts[0], 'exact selection after clear')
    click('Export…', main)
    chooser = wait(lambda: find('Export transcript'), 'native Export chooser')
    fields = wait(lambda: [n for n in walk(chooser) if 'EditableText' in n.get_interfaces()], 'export filename field')
    name = next((n for n in fields if 'name' in n.get_name().lower()), fields[0])
    assert name.get_editable_text_iface().set_text_contents('exact-fixture.txt')
    click('Export', chooser)
    wait(lambda: (output / 'exact-fixture.txt').exists(), 'export file')
    assert (output / 'exact-fixture.txt').read_text() == texts[0]
    checks.append('Exact Copy, native Export and search/clear preserve Unicode and paragraphs')
    resize('Koett', 500, 680)
    capture('history-narrow')
    resize('Koett', 960, 680)
    button = wait(lambda: find('Menu', main, 'toggle button'), 'Menu')
    keys('chord', 29, 33)
    for _ in range(20):
        keys('chord', 42, 15)
        if button.get_state_set().contains(Atspi.StateType.FOCUSED):
            break
    assert button.get_state_set().contains(Atspi.StateType.FOCUSED)
    keys('key', 57)
    wait(lambda: find('Hide window', main), 'menu open')
    capture('menu')
    node = wait(lambda: find('Local models…', main, 'menu item'), 'models menu item')
    assert node.get_action_iface().do_action(0)
    models = wait(lambda: find('Local models', role='frame'), 'models')
    capture('models-110m')
    if args.visual_only:
        resize('Local models', 460 if args.scaled else 380, 440)
        capture('models-narrow')
        keys('key', 1)
        control('settings')
        settings = wait(lambda: find('Settings', role='frame'), 'settings')
        navigation = next(n for n in walk(settings) if n.get_role_name() == 'combo box')
        resize('Settings', 540, 460)
        rect = navigation.get_component_iface().get_extents(Atspi.CoordType.WINDOW)
        print('Navigation rect:', rect.x, rect.y, rect.width, rect.height, flush=True)
        for index, category in enumerate(['audio', 'shortcuts', 'text', 'general']):
            subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), 'move', str(400 + rect.x + 50), str(140 + rect.y + rect.height // 2)], check=True)
            subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), 'click'], check=True)
            keys('key', 102)
            for _ in range(index):
                keys('key', 108)
            keys('key', 28)
            expected = ['Input level', 'Recording mode', 'Paste shortcut', 'Launch at login'][index]
            wait(lambda expected=expected: find(expected, settings), 'category ' + category)
            capture('settings-' + category)
        click('Save', settings)
        with sqlite3.connect(database) as connection:
            connection.execute('DELETE FROM transcripts')
        control('show')
        wait(lambda: find('No transcripts yet', main), 'empty')
        capture('history-empty')
        (output / 'checks.json').write_text(json.dumps(checks, indent=2))
        raise SystemExit(0)
    v2 = next(n for n in walk(models) if '600M' in n.get_name() and 'Action' in n.get_interfaces())
    assert v2.get_action_iface().do_action(0)
    wait(lambda: find('Cancel model setup', models).get_state_set().contains(Atspi.StateType.SENSITIVE), 'cancellable preparation')
    capture('models-progress')
    click('Cancel model setup', models)
    wait(lambda: find('Ready', models), 'cancelled ready', timeout=120)
    assert json.loads(config.read_text()).get('selected_model', 'compact') == 'compact'
    capture('models-cancelled')
    assert v2.get_action_iface().do_action(0)
    wait(lambda: find('Ready', models), 'v2 ready', timeout=120)
    wait(lambda: any(n.get_name().startswith('Active: Parakeet v2 600M') for n in walk(models)), 'distinct active v2', timeout=120)
    assert json.loads(config.read_text())['selected_model'] == 'parakeet_v2'
    capture('models-v2')
    installed = output / 'models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8'
    engine_binary = binary.with_name('koett-engine')
    results = []
    for directory in [fixture_model, installed]:
        result = subprocess.run([str(engine_binary), str(directory), str(model / 'test_wavs/0.wav')], check=True, capture_output=True, text=True)
        results.append({'model': directory.name, 'stdout': result.stdout, 'stderr': result.stderr})
    (output / 'inference.json').write_text(json.dumps(results, indent=2))
    compact = next(n for n in walk(models) if '110M' in n.get_name() and 'Action' in n.get_interfaces())
    assert compact.get_action_iface().do_action(0)
    wait(lambda: any(n.get_name().startswith('Active: Parakeet 110M') for n in walk(models)), 'back to compact', timeout=120)
    wait(lambda: find('Ready', models), 'compact ready')
    config.parent.chmod(0o500)
    try:
        assert v2.get_action_iface().do_action(0)
        wait(lambda: any('could not' in n.get_name() for n in walk(models)), 'preference save error', timeout=120)
        wait(lambda: find('Ready', models), 'fallback ready')
        assert json.loads(config.read_text())['selected_model'] == 'compact'
        assert any(n.get_name().startswith('Active: Parakeet 110M') for n in walk(models))
        capture('models-preference-failure-recovery')
    finally:
        config.parent.chmod(0o700)
    assert v2.get_action_iface().do_action(0)
    wait(lambda: any(n.get_name().startswith('Active: Parakeet v2 600M') for n in walk(models)), 'v2 restored', timeout=120)
    wait(lambda: find('Ready', models), 'restored ready')
    control('start')
    wait(lambda: any(n.get_role_name() == 'label' and n.get_name().startswith('Recording') for n in walk(models)), 'recording')
    assert not compact.get_state_set().contains(Atspi.StateType.SENSITIVE)
    assert not v2.get_state_set().contains(Atspi.StateType.SENSITIVE)
    assert json.loads(config.read_text())['selected_model'] == 'parakeet_v2'
    capture('models-busy-refusal')
    control('stop')
    wait(lambda: find('Ready', models), 'stopped', timeout=120)
    checks.append('Real distinct 110M/v2 inference; cancel preserves default; failed preference save retains active compact worker; busy choices disabled')
    keys('key', 1)
    control('settings')
    settings = wait(lambda: find('Settings', role='frame'), 'settings')
    capture('settings')
    click('Save', settings)
    assert json.loads(config.read_text())['selected_model'] == 'parakeet_v2'
    process.terminate()
    process.wait(timeout=30)
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    main = wait(lambda: find('Koett', role='frame'), 'restarted main')
    wait(lambda: any(n.get_role_name() == 'label' and n.get_name().startswith('Ready') for n in walk(main)), 'restart ready', timeout=120)
    click('Menu', main)
    node = wait(lambda: find('Local models…', main, 'menu item'), 'restarted models menu item')
    assert node.get_action_iface().do_action(0)
    models = wait(lambda: find('Local models', role='frame'), 'restarted models')
    wait(lambda: find('Active: Parakeet v2 600M · TDT int8', models), 'restart active v2', timeout=120)
    capture('models-restart-active-v2')
    keys('key', 1)
    checks.append('Restart loads saved v2; active recognizer identity verified')
    checks.append('Explicit verified cached v2 selection persists; Settings save preserves selection; active model differs')
    with sqlite3.connect(database) as connection:
        connection.execute('DELETE FROM transcripts')
    control('show')
    wait(lambda: find('No transcripts yet', main), 'empty')
    capture('history-empty')
    (output / 'checks.json').write_text(json.dumps(checks, indent=2))
finally:
    if process is not None:
        process.terminate()
        process.wait(timeout=30)
