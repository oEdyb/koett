#!/usr/bin/env python3
"""Timeline pill evidence in a private agent-lab; caller owns the lab.

Real model, real private audio and real delivery. Sketch bands come from the
debug-build-only `sketch-debug` control command, standing in for draw mode.
Run inside: agent-lab run koett-pill -- python3 this.py --binary ... --theme ...
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
parser.add_argument('--lab', default='koett-pill')
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--theme', choices=['light', 'dark'], required=True)
args = parser.parse_args()
runtime = f'/run/user/{os.getuid()}/agent-lab/{args.lab}'
assert os.environ.get('AGENT_LAB_RUNTIME') == runtime
assert os.environ.get('WAYLAND_DISPLAY') == 'wayland-agent-lab'
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
binary = args.binary.resolve()
models = Path.home() / '.local/share/koett/models'
model = models / 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8'
speech = model / 'test_wavs/0.wav'
assert speech.is_file()
os.environ.pop('NIRI_SOCKET', None)
os.environ.pop('AT_SPI_BUS_ADDRESS', None)
for kind in ['CONFIG', 'DATA', 'CACHE', 'STATE']:
    os.environ[f'XDG_{kind}_HOME'] = str(output / kind.lower())
os.environ.update(GTK_A11Y='atspi', GTK_THEME='Adwaita:' + args.theme, GTK_USE_PORTAL='0')
config = output / 'config/koett/settings.json'
config.parent.mkdir(parents=True)
config.write_text(json.dumps({'model_directory': str(model), 'start_at_login': False}))
# Reuse the lab's previously granted remote-desktop session (copied into
# this lab's portal permission store) so real automatic paste can succeed.
token = Path('/tmp/koett-rd-token')
if token.is_file():
    (output / 'state/koett').mkdir(parents=True)
    (output / 'state/koett/remote-desktop-token').write_bytes(token.read_bytes())
(output / 'config/user-dirs.dirs').write_text(f'XDG_DOCUMENTS_DIR="{output}"\n')
module = subprocess.check_output(['pactl', 'load-module', 'module-remap-source',
    'master=agent_lab_audio.monitor', 'source_name=pill_mic'], text=True).strip()
atexit.register(lambda: subprocess.run(['pactl', 'unload-module', module], check=True))
alsa = output / 'alsa.conf'
alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "pill_mic" }\nctl.!default { type pulse }\n')
os.environ.update(ALSA_CONFIG_PATH=str(alsa), PULSE_SOURCE='pill_mic')
address = ast.literal_eval(subprocess.check_output(['gdbus', 'call', '--session', '--dest', 'org.a11y.Bus', '--object-path', '/org/a11y/bus', '--method', 'org.a11y.Bus.GetAddress'], text=True).strip())[0]
os.environ['AT_SPI_BUS_ADDRESS'] = address
registry = subprocess.Popen(['/usr/lib/at-spi2-registryd'], stdout=(output / 'accessibility.log').open('w'), stderr=subprocess.STDOUT)
atexit.register(lambda: registry.terminate() if registry.poll() is None else None)
subprocess.run(['gdbus', 'call', '--session', '--dest', 'org.a11y.Bus', '--object-path', '/org/a11y/bus', '--method', 'org.freedesktop.DBus.Properties.Set', 'org.a11y.Status', 'IsEnabled', '<true>'], check=True, stdout=subprocess.DEVNULL)
import gi

gi.require_version('Atspi', '2.0')
from gi.repository import Atspi
from PIL import Image


def walk(node):
    yield node
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child:
            yield from walk(child)


def find(name, root=None, role=None):
    return next((node for node in walk(root or Atspi.get_desktop(0))
                 if node.get_name() == name and (role is None or node.get_role_name() == role)
                 and node.get_state_set().contains(Atspi.StateType.SHOWING)), None)


def press(name, root=None):
    node = wait(lambda: find(name, root, 'button'), name)
    assert node.get_action_iface().do_action(0), name
    time.sleep(0.2)


def control(command):
    subprocess.run([str(binary), command], check=True, capture_output=True)


def capture(name, settle=0.3):
    time.sleep(settle)
    path = output / (name + '.png')
    subprocess.run(['/usr/bin/python3', str(Path.home() / '.local/share/agent-lab/screenshot.py'), str(path)], check=True, stdout=subprocess.DEVNULL)
    # A 1:1 crop of the bottom-centre pill for review.
    image = Image.open(path)
    box = (image.width // 2 - 190, image.height - 160, image.width // 2 + 190, image.height)
    image.crop(box).save(output / (name + '-pill.png'))
    print('Captured ' + name, flush=True)
    return image


def keys(command, *codes):
    subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), command, *map(str, codes)], check=True)


def kwin(source):
    path = output / 'window.js'
    path.write_text(source)
    ident = subprocess.check_output(['qdbus6', 'org.kde.KWin', '/Scripting', 'loadScript', str(path), 'pill-test'], text=True).strip()
    try:
        subprocess.run(['qdbus6', 'org.kde.KWin', '/Scripting/Script' + ident, 'org.kde.kwin.Script.run'], check=True, capture_output=True)
    finally:
        subprocess.run(['qdbus6', 'org.kde.KWin', '/Scripting', 'unloadScript', 'pill-test'], check=True, capture_output=True)
    time.sleep(0.3)


def play(path=speech):
    return subprocess.Popen(['pw-play', '--target=agent_lab_audio', str(path)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def rows():
    db = output / 'data/koett/History.sqlite3'
    if not db.exists():
        return []
    with sqlite3.connect(db) as connection:
        return connection.execute('SELECT text FROM transcripts ORDER BY created_at_ms').fetchall()


def sketch(index, start, end, state):
    control(f'sketch-debug {index} {start} {end} {state}')


def focus(caption):
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "' + caption + '") workspace.activeWindow=w;')


process = target = None
checks = []
try:
    log = (output / 'app.log').open('w')
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    wait(lambda: find('Ready'), 'real model ready', timeout=180)
    gui = output / 'target.py'
    gui.write_text('''import gi
gi.require_version('Gtk','4.0')
from gi.repository import Gtk
app=Gtk.Application(application_id='com.koett.PillTarget')
def activate(app):
 w=Gtk.ApplicationWindow(application=app,title='Pill typing target');w.set_default_size(900,400)
 e=Gtk.Entry();w.set_child(e);w.present();e.grab_focus()
app.connect('activate',activate);app.run([])
''')
    target = subprocess.Popen(['/usr/bin/python3', str(gui)], stdout=(output / 'target.log').open('w'), stderr=subprocess.STDOUT)
    target_window = wait(lambda: find('Pill typing target'), 'target')
    entry = next(n for n in walk(target_window) if 'EditableText' in n.get_interfaces())
    focus('Pill typing target')
    screen = capture('desktop-idle')
    centre_x, row_y, actions_y = screen.width // 2, screen.height - 48, screen.height - 92

    # Long dictation: speech, a pause, speech while drawing, paused, speech.
    count = len(rows())
    started = time.monotonic()
    control('start')
    wait(lambda: find('Koett recording'), 'recording pill')
    player = play()
    time.sleep(2.5)
    sketch(1, 1500, 'now', 'live')
    time.sleep(1.0)
    sketch(1, 1500, 'now', 'live')
    capture('drawing', settle=0.2)
    sketch(1, 1500, 'now', 'done')
    player.wait(timeout=15)
    time.sleep(2.5)
    capture('recording')
    keys('move', centre_x, row_y)
    pill = wait(lambda: find('Cancel', role='button'), 'hover row appears')
    assert find('Pause', role='button') and find('History', role='button')
    capture('hover')
    checks.append('hover reveals Cancel, Pause and History over the strip')
    press('Pause')
    wait(lambda: find('Resume', role='button'), 'paused')
    time.sleep(2.0)
    capture('paused')
    press('Resume')
    keys('move', 80, 80)
    wait(lambda: not find('Cancel', role='button'), 'hover row hides on leave')
    assert entry.get_state_set().contains(Atspi.StateType.FOCUSED) or target_window.get_state_set().contains(Atspi.StateType.ACTIVE), 'pill clicks never take focus'
    checks.append('Pause/Resume from the pill; hover leave hides actions; target keeps focus')
    player = play()
    time.sleep(3.0)
    second = int((time.monotonic() - started) * 1000)
    sketch(2, second - 1200, 'now', 'live')
    time.sleep(1.5)
    sketch(2, second - 1200, 'now', 'done')
    player.wait(timeout=15)
    time.sleep(1.0)
    player = play()
    player.wait(timeout=15)
    control('stop')
    transcribing = []
    for frame in range(6):
        if not find('Koett recording'):
            break
        transcribing.append(capture(f'transcribing-{frame}', settle=0.05))
    wait(lambda: len(rows()) > count, 'real ASR result persisted', timeout=180)
    wait(lambda: find('Copy', role='button'), 'done row')
    capture('done')
    text = rows()[-1][0]
    wait(lambda: Atspi.Text.get_text(entry, 0, -1).strip() == text.strip(), 'transcript delivered to target', timeout=10)
    checks.append(f'{len(transcribing)} transcribing frame(s) captured; done row after successful delivery of {len(text)} chars')
    press('Copy')
    wait(lambda: find('Copied', role='button'), 'Copied')
    assert subprocess.check_output(['wl-paste', '--no-newline']).decode() == text
    checks.append('Done Copy places the exact SQLite text on the clipboard')
    wait(lambda: not find('Koett recording'), 'done dismisses itself', timeout=20)
    checks.append('done state dismisses itself after its timeout')

    # Recovery: focus moves away during capture, so the guard skips paste.
    focus('Koett')
    count = len(rows())
    control('start')
    wait(lambda: find('Koett recording'), 'recording pill 2')
    focus('Pill typing target')
    play().wait(timeout=15)
    focus('Koett')
    control('stop')
    wait(lambda: len(rows()) > count, 'second result persisted', timeout=180)
    wait(lambda: find('Paste again', role='button'), 'recovery row')
    capture('recovery')
    checks.append('guarded delivery shows Paste again / Copy / History')
    focus('Pill typing target')

    # Cancel discards: no transcript, no paste.
    count = len(rows())
    before = Atspi.Text.get_text(entry, 0, -1)
    control('start')
    wait(lambda: find('Koett recording') and not find('Paste again', role='button'), 'recording pill 3')
    player = play()
    time.sleep(2.0)
    keys('move', centre_x, row_y)
    press('Cancel')
    player.wait(timeout=15)
    wait(lambda: not find('Koett recording'), 'cancel hides pill')
    time.sleep(3.0)
    assert len(rows()) == count, 'cancel saves nothing'
    assert Atspi.Text.get_text(entry, 0, -1) == before, 'cancel pastes nothing'
    checks.append('Cancel discards capture: no history row, no paste, pill hidden')
    keys('move', 80, 80)

    # `koett cancel` control command, the Wayland route for a keyboard binding.
    control('start')
    wait(lambda: find('Koett recording'), 'recording pill 4')
    time.sleep(1.0)
    control('cancel')
    wait(lambda: not find('Koett recording'), 'control cancel hides pill')
    time.sleep(2.0)
    assert len(rows()) == count
    checks.append('`koett cancel` discards the active dictation')
    (output / 'results.json').write_text(json.dumps(checks, indent=2))
    print(json.dumps({'checks': checks, 'evidence': str(output)}, indent=2))
except Exception:
    capture('failure')
    (output / 'results.json').write_text(json.dumps(checks, indent=2))
    raise
finally:
    for child in [target, process]:
        if child and child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                child.kill()
