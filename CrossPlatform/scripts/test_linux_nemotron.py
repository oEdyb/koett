#!/usr/bin/env python3
"""Private native Nemotron selection, capture, history and restart acceptance."""
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
    keys('move', 10, 10)
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



(cache / 'sherpa-onnx-nemotron-speech-streaming-en-0.6b-560ms-int8-2026-04-25.tar.bz2').symlink_to('/tmp/koett-nemotron-eval/model.tar.bz2')
v2_name = 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8'
(output / 'models' / v2_name).symlink_to(Path('/tmp/koett-parity-models-acceptance/models') / v2_name, target_is_directory=True)
nemotron_name = 'sherpa-onnx-nemotron-speech-streaming-en-0.6b-560ms-int8-2026-04-25'
verified_cache = Path('/tmp/koett-nemotron-native-light3/models') / nemotron_name
if verified_cache.is_dir():
    (output / 'models' / nemotron_name).symlink_to(verified_cache, target_is_directory=True)
process = None
try:
    log = (output / 'app.log').open('w')
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    main = wait(lambda: find('Koett', role='frame'), 'main')
    wait(lambda: any(n.get_name().startswith('Ready') for n in walk(main)), 'ready', timeout=120)
    click('Menu', main)
    click('Local models…', main)
    models = wait(lambda: find('Local models', role='frame'), 'models')
    nemotron = next(n for n in walk(models) if 'Nemotron 0.6B' in n.get_name() and 'Action' in n.get_interfaces())
    assert nemotron.get_action_iface().do_action(0)
    wait(lambda: any('Active: Nemotron 0.6B' in n.get_name() for n in walk(models)), 'active Nemotron', timeout=180)
    wait(lambda: find('Ready', models), 'ready Nemotron')
    assert json.loads(config.read_text())['selected_model'] == 'nemotron'
    print(Path(f'/proc/{process.pid}/status').read_text(), flush=True)
    capture('models-nemotron')
    resize('Local models', 380, 440)
    capture('models-narrow')
    for iteration in range(2):
        with sqlite3.connect(database) as c:
            before = c.execute('SELECT count(*) FROM transcripts').fetchone()[0]
        control('start')
        wait(lambda: current_streams(), 'native CPAL stream')
        wait(lambda: not nemotron.get_state_set().contains(Atspi.StateType.SENSITIVE), 'busy choices disabled')
        if iteration == 0:
            capture('models-busy')
        subprocess.run(['paplay', '--device=agent_lab_audio', '/tmp/koett-nemotron-eval/sherpa-onnx-nemotron-speech-streaming-en-0.6b-560ms-int8-2026-04-25/test_wavs/0.wav'], check=True)
        started = time.monotonic()
        control('stop')
        def result(before=before):
            with sqlite3.connect(database) as c:
                rows = c.execute('SELECT model,text FROM transcripts ORDER BY created_at_ms DESC LIMIT 1').fetchall()
                count = c.execute('SELECT count(*) FROM transcripts').fetchone()[0]
            return rows[0] if count == before + 1 else None
        row = wait(result, 'single final history result', timeout=120)
        assert 'nemotron' in row[0] and 'yellow lamps' in row[1].lower(), row
        wait(lambda: find('Ready', models), 'recording finalized')
        assert not current_streams()
        assert not list((output / 'data').rglob('*.wav'))
        print(json.dumps({'iteration': iteration, 'stop_to_history_s': time.monotonic() - started, 'result': row}), flush=True)
    capture('models-after-repeat')
    compact = next(n for n in walk(models) if '110M' in n.get_name() and 'Action' in n.get_interfaces())
    config.parent.chmod(0o500)
    try:
        assert compact.get_action_iface().do_action(0)
        wait(lambda: any('could not' in n.get_name() for n in walk(models)), 'save failure', timeout=120)
        wait(lambda: find('Ready', models), 'worker retained')
        assert json.loads(config.read_text())['selected_model'] == 'nemotron'
        assert any('Active: Nemotron' in n.get_name() for n in walk(models))
        capture('models-error-retained-nemotron')
    finally:
        config.parent.chmod(0o700)
    assert nemotron.get_action_iface().do_action(0)
    wait(lambda: find('Cancel model setup', models).get_state_set().contains(Atspi.StateType.SENSITIVE), 'cancel available')
    click('Cancel model setup', models)
    wait(lambda: find('Ready', models), 'cancelled preparation', timeout=120)
    assert json.loads(config.read_text())['selected_model'] == 'nemotron'
    capture('models-cancel-retained-nemotron')
    process.terminate(); process.wait(timeout=30)
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    main = wait(lambda: find('Koett', role='frame'), 'restart')
    wait(lambda: any(n.get_name().startswith('Ready') for n in walk(main)), 'restart ready', timeout=120)
    click('Menu', main); click('Local models…', main)
    models = wait(lambda: find('Local models', role='frame'), 'models')
    wait(lambda: any('Active: Nemotron 0.6B' in n.get_name() for n in walk(models)), 'restart identity')
    capture('models-restarted')
    keys('key', 1)
    click('Menu', main)
    wait(lambda: find('Failed recordings…', main), 'recovery action')
    keys('key', 15)
    capture('recovery-menu-keyboard')
    (output / 'checks.json').write_text(json.dumps({'native_cpal_nemotron_repeat': True, 'history_once': True, 'no_success_wav': True, 'saved_restart_identity': True}))
finally:
    if process is not None:
        process.terminate(); process.wait(timeout=30)
