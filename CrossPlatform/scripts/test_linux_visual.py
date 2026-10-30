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
model = Path.home() / '.local/share/koett/models/sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8'
assert model.is_dir()
config.write_text(json.dumps({'model_directory': str(model), 'start_at_login': False}))
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
    time.sleep(0.3)
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
    main = wait(lambda: find('Koett', role='frame'), 'main window')
    wait(lambda: transcript() == texts[0], 'exact long Unicode selection')
    wait(lambda: find('Record', main), 'ready model', timeout=120)
    resize('Koett', 960, 680)
    capture('history-populated-normal')
    keys('chord', 29, 33)
    search_entry = find('Search transcripts', main, 'text')
    assert search_entry.get_state_set().contains(Atspi.StateType.FOCUSED)
    capture('history-keyboard-focus')
    click('Copy', main)
    wait(lambda: subprocess.check_output(['wl-paste', '--no-newline']).decode() == texts[0], 'long Unicode exact Copy')
    checks.append('Ctrl+F focuses named native search; exact long Unicode transcript and Copy')
    for width, suffix in [(960, 'normal'), (500, 'narrow')]:
        resize('Koett', width, 680)
        capture('history-populated-' + suffix + '-list')
        history_list = next(n for n in walk(main) if n.get_role_name() == 'list')
        assert history_list.get_selection_iface().select_child(1)
        assert history_list.get_selection_iface().select_child(0)
        wait(lambda: transcript() == texts[0], 'selected exact original')
        capture('history-populated-' + suffix + '-detail')
        if suffix == 'narrow':
            click('Back to history', main)
            capture('history-populated-narrow-list-returned')
        search('no-matching-synthetic-text')
        wait(lambda: find('No matches', main), 'no results')
        capture('history-no-results-' + suffix)
        click('Clear search', main)
    resize('Koett', 960, 680)
    checks.append('normal and ' + ('560px enlarged-font' if args.scaled else '500px') + ' list/detail navigation and no-results recovery')
    with sqlite3.connect(database) as connection:
        connection.execute("UPDATE transcripts SET model=? WHERE id='visual-0'", ('Synthetic-long-model-name-' + 'multilingual-' * 15 + '日本語',))
    control('show')
    wait(lambda: any(n.get_role_name() == 'label' and 'Synthetic-long-model-name-' in n.get_name() for n in walk(main)), 'long model metadata')
    capture('history-long-model-normal')
    resize('Koett', 500, 680)
    history_list = next(n for n in walk(main) if n.get_role_name() == 'list')
    assert history_list.get_selection_iface().select_child(1)
    assert history_list.get_selection_iface().select_child(0)
    capture('history-long-model-narrow')
    click('Back to history', main)
    resize('Koett', 960, 680)
    control('settings')
    settings = wait(lambda: find('Settings', role='frame'), 'Settings')
    for section in ['Audio', 'Shortcuts', 'Text', 'General']:
        click(section, settings)
        assert find('Save', settings)
        if section != 'Audio':
            wait(lambda: not current_streams(), 'hidden Audio releases input')
        capture('settings-' + section.lower())
    resize('Settings', 500, 460)
    for section in ['Audio', 'Shortcuts', 'Text', 'General']:
        click(section, settings)
        assert find('Save', settings)
        capture('settings-' + section.lower() + '-narrow')
    click('Close', settings)
    wait(lambda: not find('Settings', role='frame'), 'Settings closed')
    wait(lambda: not current_streams(), 'Settings close releases input')
    saved = json.loads(config.read_text())
    saved['microphone'] = 'Synthetic unavailable microphone with an exceptionally long device name — Åäö 日本語 ' * 3
    saved['replacements'] = [{'phrase': f'synthetic phrase {i}', 'replacement': f'Exact replacement {i} åäö 日本語'} for i in range(16)]
    config.write_text(json.dumps(saved))
    control('settings')
    settings = wait(lambda: find('Settings', role='frame'), 'long Settings')
    wait(lambda: any('Selected microphone unavailable' in n.get_name() for n in walk(settings)), 'explicit unavailable input')
    resize('Settings', 540, 460)
    capture('settings-long-input')
    click('Text', settings)
    capture('settings-long-rules')
    phrases = [n for n in walk(settings) if n.get_name() == 'Phrase' and 'EditableText' in n.get_interfaces()]
    assert len(phrases) == 16
    for _ in range(60):
        phrases[0].clear_cache()
        if phrases[0].get_state_set().contains(Atspi.StateType.FOCUSED):
            break
        keys('key', 15)
        time.sleep(0.05)
    phrases[0].clear_cache()
    assert phrases[0].get_state_set().contains(Atspi.StateType.FOCUSED)
    keys('key', *([15] * 48))
    wait(lambda: find('Add replacement', settings), 'keyboard scrolls to last replacement')
    assert find('Save', settings)
    capture('settings-long-rules-bottom')
    click('General', settings)
    click('Save', settings)
    assert json.loads(config.read_text())['replacements'] == saved['replacements']
    checks.append('all Settings pages; long names/rules; Save remains reachable and exact rules persist')
    with sqlite3.connect(database) as connection:
        connection.execute('DELETE FROM transcripts')
    control('show')
    wait(lambda: find('No transcripts yet', main), 'empty history')
    for width, suffix in [(960, 'normal'), (500, 'narrow')]:
        resize('Koett', width, 680)
        capture('history-empty-' + suffix)
    control('quit')
    process.wait(timeout=15)
    database.rename(database.with_suffix('.fixture-backup'))
    database.mkdir()
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    main = wait(lambda: find('Koett', role='frame'), 'error main')
    wait(lambda: any(n.get_role_name() == 'label' and n.get_name().startswith('History unavailable:') for n in walk(main)), 'history error')
    for width, suffix in [(960, 'normal'), (500, 'narrow')]:
        resize('Koett', width, 680)
        capture('history-error-' + suffix)
        assert find('Retry', main)
    database.rmdir()
    database.with_suffix('.fixture-backup').rename(database)
    click('Retry', main)
    wait(lambda: find('No transcripts yet', main), 'history retry recovers')
    checks.append('empty and real database-open error at normal/narrow sizes; Retry recovers')
    (output / 'results.json').write_text(json.dumps(checks, indent=2))
    print(json.dumps(checks, indent=2))
except Exception:
    capture('failure')
    raise
finally:
    if process is not None and process.poll() is None:
        control('quit')
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=5)
