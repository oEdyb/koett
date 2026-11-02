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
    assert node.get_component_iface().grab_focus()
    keys('key', 57)
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

# This integration intentionally stops at the unavailable private key store.
# It never supplies a cloud key, endpoint, or prerecorded screenshot.
config.with_name('assistant.json').write_text(json.dumps({
    'enabled': True, 'provider': 'groq',
    'endpoint': 'https://api.groq.com/openai/v1/chat/completions',
    'model': 'qwen/qwen3.6-27b'}))
process = None
try:
    log = (output / 'app.log').open('w')
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    main = wait(lambda: find('Koett', role='frame'), 'main')
    wait(lambda: find('Record', main) and find('Record', main).get_state_set().contains(Atspi.StateType.SENSITIVE), 'ready', timeout=120)
    control('ask-start')
    time.sleep(2)
    capture('screen-permission')
    # All coordinates address the private lab compositor exclusively.
    subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), 'move', '951', '411'], check=True)
    subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), 'click'], check=True)
    panel = wait(lambda: find('Koett Ask', role='frame'), 'Ask panel', timeout=20)
    wait(lambda: find('Listening…', panel), 'listening', timeout=20)
    assert find('Screen included', panel)
    capture('ask-listening')
    subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    control('ask-stop')
    wait(lambda: find('Could not answer', panel), 'private key-store failure', timeout=25)
    capture('ask-secret-error')
    resize('Koett Ask', 440 if args.scaled else 360, 500)
    capture('ask-secret-error-narrow')
    with sqlite3.connect(output / 'data/koett/History.sqlite3') as connection:
        questions = connection.execute('SELECT text FROM transcripts').fetchall()
    assert len(questions) == 1 and 'old portrait' in questions[0][0].lower(), questions
    status = json.loads(subprocess.check_output([str(binary), 'ask-status']))
    assert status['phase'] == 'Error', status
    assert 'question' not in status and 'answer' not in status
    control('ask-cancel')
    control('start')
    wait(lambda: find('Stop', main), 'dictation after Ask')
    subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    control('stop')
    wait(lambda: find('Record', main), 'dictation completed', timeout=120)
    with sqlite3.connect(output / 'data/koett/History.sqlite3') as connection:
        rows = connection.execute('SELECT text FROM transcripts').fetchall()
    assert len(rows) == 2 and all('old portrait' in row[0].lower() for row in rows), rows
    capture('dictation-after-ask')
    (output / 'checks.json').write_text(json.dumps({'real_voice_question_saved_once': True, 'private_portal_frame': True, 'secret_service_failure_visible': True, 'normal_dictation_after_ask': True, 'cloud_requests': 0}))
finally:
    if process is not None:
        subprocess.run([str(binary), 'quit'], timeout=10, check=False)
        try:
            process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=5)
    registry.terminate()
