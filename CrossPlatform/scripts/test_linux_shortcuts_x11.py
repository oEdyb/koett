#!/usr/bin/env python3
"""Actual X11 shortcut acceptance in a private rootful Xwayland server.

Uses only its private compositor, accessibility bus, clipboard, synthetic audio,
settings and history. No app test hooks or model downloads.
"""
import argparse
import ast
import atexit
import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
import time
from pathlib import Path

from test_linux_ui import wait

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--capture-lifecycle', action='store_true')
parser.add_argument('--lab', default='koett-shortcuts-x11-owned')
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--theme', choices=['light', 'dark'], required=True)
parser.add_argument('--renderer', choices=['default', 'cairo', 'gl'], default='default')

args = parser.parse_args()
assert shutil.which("xclip"), "X11 recovery requires xclip in private process PATH"
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
xwayland = subprocess.Popen(['Xwayland', '-displayfd', '1', '-ac', '-noreset',
    '-nolisten', 'tcp', '-geometry', '1500x1000', '-glamor', 'off'],
    stdout=subprocess.PIPE, stderr=(output / 'xwayland.log').open('w'), text=True)
private_display = ':' + xwayland.stdout.readline().strip()
assert private_display != ':' and private_display != os.environ.get('DISPLAY')
atexit.register(lambda: xwayland.terminate() if xwayland.poll() is None else None)
os.environ['DISPLAY'] = private_display
os.environ.pop('WAYLAND_DISPLAY')
os.environ['GDK_BACKEND'] = 'x11'
os.environ['XDG_SESSION_TYPE'] = 'x11'
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
from PIL import Image
from Xlib import XK, X, display

connection = display.Display(private_display)
root_window = connection.screen().root


def walk(node):
    yield node
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child:
            yield from walk(child)


def find(name, role=None, within=None):
    for node in walk(within or Atspi.get_desktop(0)):
        node.clear_cache()
        if node.get_name() == name and (role is None or node.get_role_name() == role) and node.get_state_set().contains(Atspi.StateType.SHOWING):
            return node
    return None


def click(name):
    node = wait(lambda: find(name), name)
    assert node.get_action_iface().do_action(0), name
    time.sleep(0.25)
    return node


def control(command):
    result = subprocess.run([str(binary), command], check=True, capture_output=True, text=True)
    return result.stdout


def state():
    return json.loads(control('shortcuts-status'))


def key(*parts):
    subprocess.run(['xdotool', *parts], check=True)


def focus(title):
    ids = subprocess.check_output(['xdotool', 'search', '--onlyvisible', '--name', '^' + title + '$'], text=True).split()
    key('windowraise', ids[-1])
    key('windowfocus', '--sync', ids[-1])


def capture(name):
    time.sleep(0.4)
    geometry = root_window.get_geometry()
    pixels = root_window.get_image(0, 0, geometry.width, geometry.height, X.ZPixmap, 0xffffffff)
    Image.frombytes('RGB', (geometry.width, geometry.height), pixels.data, 'raw', 'BGRX').save(output / (name + '.png'))


def update(**values):
    settings = json.loads(config.read_text())
    settings.update(values)
    pending = config.with_suffix('.pending')
    pending.write_text(json.dumps(settings))
    pending.replace(config)
    time.sleep(1.2)


def settings_page():
    control('settings')
    wait(lambda: find('Settings', 'frame'), 'Settings')
    focus('Settings')
    ident = subprocess.check_output(['xdotool', 'search', '--name', '^Settings$'], text=True).split()[-1]
    key('windowsize', ident, '540', '900')
    combo = wait(lambda: find('Settings section', 'combo box'), 'Settings selector')
    assert combo.get_selection_iface().select_child(1), 'select shortcut page'
    wait(lambda: find('Recording mode'), 'shortcut page')


def close_settings():
    window_id = subprocess.check_output(['xdotool', 'search', '--name', '^Settings$'], text=True).split()[-1]
    connection.create_resource_object('window', int(window_id)).send_event(
        __import__('Xlib.protocol.event', fromlist=['ClientMessage']).ClientMessage(
            window=int(window_id), client_type=connection.intern_atom('WM_PROTOCOLS'),
            data=(32, [connection.intern_atom('WM_DELETE_WINDOW'), X.CurrentTime, 0, 0, 0])), event_mask=0)
    connection.flush()
    wait(lambda: not find('Settings', 'frame'), 'Settings close')


def bind(label, chord):
    label_node = wait(lambda: find(label, 'label', find('Settings', 'frame')), label)
    row = label_node.get_parent()
    button = row.get_child_at_index(1)
    old = button.get_child_at_index(0).get_name()
    assert button.get_action_iface().do_action(0)
    wait(lambda: find('Press your shortcut…'), 'capture ready')
    focus('Settings')
    key('key', chord)
    wait(lambda: button.get_child_at_index(0).get_name() != 'Press your shortcut…', 'capture complete')
    return old, button.get_child_at_index(0).get_name()


def ready():
    wait(lambda: find('Ready'), 'Ready after capture', timeout=100)


def recording():
    wait(lambda: find('Stop'), 'actual X11 Recording', timeout=30)


def chord(value):
    key('key', value)


def toggle(value):
    chord(value)
    recording()
    chord(value)
    ready()


def launch():
    child = subprocess.Popen(['taskset', '--cpu-list', str(min(os.sched_getaffinity(0))), str(binary)],
        stdout=(output / 'app.log').open('a'), stderr=subprocess.STDOUT)
    wait(lambda: find('Ready'), 'real model ready', timeout=120)
    wait(lambda: state().get('backend') == 'x11', 'X11 backend')
    return child


process = None
class Checks(list):
    def append(self, value):
        super().append(value)
        print(value, flush=True)


checks = Checks()
proof = {'binary': str(binary), 'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
         'display': private_display, 'checks': checks}
try:
    process = launch()
    if args.capture_lifecycle:
        settings_page()
        button = find('Dictation', 'button', find('Settings', 'frame'))
        assert button.get_action_iface().do_action(0)
        wait(lambda: find('Press your shortcut…'), 'first recorder activation')
        assert button.get_action_iface().do_action(0)
        time.sleep(0.3)
        wait(lambda: find('Press your shortcut…'), 'second recorder activation')
        focus('Settings')
        chord('ctrl+alt+space')
        wait(lambda: button.get_child_at_index(0).get_name() == 'Ctrl+Alt+Space', 'double-click recorder receives grabbed chord')
        assert not find('Press your shortcut…')
        toggle('ctrl+shift+space')
        checks.append('Double-clicking native recorder still captures grabbed chord and releases editing')
        assert button.get_action_iface().do_action(0)
        wait(lambda: find('Press your shortcut…'), 'Dictation capture before Ask Reset')
        reset_buttons = [n for n in walk(find('Settings', 'frame')) if n.get_name() == 'Reset' and n.get_role_name() == 'button']
        assert len(reset_buttons) == 3
        assert reset_buttons[1].get_action_iface().do_action(0)
        wait(lambda: not find('Press your shortcut…'), 'Ask Reset cancels other active recorder')
        toggle('ctrl+shift+space')
        checks.append('Ask Reset cancels active Dictation recorder; old global shortcut still records')
        assert button.get_action_iface().do_action(0)
        wait(lambda: find('Press your shortcut…'), 'capture before close')
        close_settings()
        settings_page()
        old, captured = bind('Dictation', 'ctrl+alt+space')
        assert old == 'Ctrl+Shift+Space' and captured == 'Ctrl+Alt+Space', (old, captured)
        assert not find('Press your shortcut…')
        capture('capture-lifecycle-complete')
        close_settings()
        toggle('ctrl+shift+space')
        checks.append('Closing active recorder then reopening allows capture; unsaved changes remain discarded')
        control('quit')
        process.wait(timeout=15)
        assert process.returncode == 0
    else:
        initial = state()
        assert initial['ok'] and all(initial['active']), initial
        proof['initial'] = initial
        toggle('ctrl+shift+space')
        checks.append('Actual default X11 Dictation shortcut starts and stops recording')
        focus('Koett')
        key('keydown', 'ctrl+shift+space')
        recording()
        time.sleep(2.0)
        assert find('Stop'), 'autorepeat must not toggle recording off'
        key('keyup', 'ctrl+shift+space')
        time.sleep(0.5)
        assert find('Stop'), 'Toggle release must preserve recording'
        chord('ctrl+shift+space')
        ready()
        checks.append('Toggle autorepeat and release preserve recording until next press')
        chord('ctrl+alt+space')
        wait(lambda: find('Enable Ask in provider configuration first'), 'Ask disabled dispatch')
        assert json.loads(control('ask-status'))['phase'] == 'Error'
        control('ask-cancel')
        wait(lambda: json.loads(control('ask-status'))['phase'] == 'Cancelled', 'Ask cancel')
        checks.append('Actual Ask shortcut dispatches disabled-provider Error; cancel returns Cancelled')
        chord('ctrl+shift+t')
        wait(lambda: find('Transcribe media', 'frame'), 'actual media shortcut opens window')
        capture('media-shortcut')
        focus('Transcribe media')
        key('key', 'Escape')
        control('settings')
        settings_page()
        capture('settings-shortcuts')
        old, restored = bind('Dictation', 'Escape')
        assert old == restored == 'Ctrl+Shift+Space', (old, restored)
        toggle('ctrl+shift+space')
        checks.append('Native capture Escape restores label and releases SHORTCUT_EDITING: shortcut records')
        bind('Dictation', 'ctrl+shift+F8')
        click('Save')
        wait(lambda: not find('Settings', 'frame'), 'native Save closes Settings')
        wait(lambda: state().get('desired') == ['Ctrl+Shift+F8', 'Ctrl+Alt+Space', 'Ctrl+Shift+T'], 'native new binding registered')
        toggle('ctrl+shift+F8')
        settings_page()
        assert find('Ctrl+Shift+F8', 'label')
        bind('Ask', 'ctrl+shift+F7')
        bind('Media', 'ctrl+shift+F7')
        before = config.read_bytes()
        click('Save')
        wait(lambda: find('Media conflicts with Ask. Choose separate shortcuts.'), 'collision validation')
        assert config.read_bytes() == before, 'collision must not persist'
        capture('settings-collision')
        checks.append('Native capture Save/reopen new binding activates; staged duplicate Save rejects without persistence')
        close_settings()
        settings_page()
        bind('Dictation', 'ctrl+alt+space')
        bind('Ask', 'ctrl+shift+F8')
        capture('native-swapped-before-save')
        click('Save')
        wait(lambda: not find('Settings', 'frame'), 'native swapped Save')
        wait(lambda: state().get('desired') == ['Ctrl+Alt+Space', 'Ctrl+Shift+F8', 'Ctrl+Shift+T'], 'swap registered')
        toggle('ctrl+alt+space')
        chord('ctrl+shift+F8')
        wait(lambda: json.loads(control('ask-status'))['phase'] == 'Error', 'swapped Ask action')
        control('ask-cancel')
        checks.append('Native capture and Save swap already grabbed Dictation and Ask; both new actions activate correctly')
        update(recording_mode='hold')
        focus('Koett')
        key('keydown', 'ctrl+alt+space')
        recording()
        time.sleep(2)
        assert find('Stop'), 'Hold autorepeat must keep recording'
        key('keyup', 'ctrl+alt+space')
        ready()
        checks.append('Hold starts on press, survives autorepeat, stops on release')
        settings_page()
        key('keydown', 'ctrl+alt+space')
        recording()
        button = find('Dictation', 'button', find('Settings', 'frame'))
        assert button.get_action_iface().do_action(0)
        wait(lambda: find('Press your shortcut…'), 'capture while holding dictation')
        key('keyup', 'ctrl+alt+space')
        ready()
        key('key', 'Escape')
        wait(lambda: not find('Press your shortcut…'), 'Escape after hold release')
        close_settings()
        checks.append('Existing Hold release still stops recording after entering native shortcut capture')
        update(recording_mode='toggle')
        blocker = display.Display(private_display)
        blocker.screen().root.grab_key(blocker.keysym_to_keycode(XK.string_to_keysym('F9')),
            X.ControlMask | X.ShiftMask, False, X.GrabModeAsync, X.GrabModeAsync)
        blocker.sync()
        update(shortcut='Ctrl+Shift+F9', pause_media=True)
        wait(lambda: json.loads(config.read_text())['shortcut'] == 'Ctrl+Alt+Space', 'occupied binding rollback')
        failed = state()
        proof['occupied_failure'] = failed
        assert not failed['ok'] and 'previous bindings retained' in failed['error']['message'], failed
        assert json.loads(config.read_text())['pause_media'] is True
        toggle('ctrl+alt+space')
        blocker.close()
        capture('occupied-key-rollback')
        checks.append('Occupied XGrabKey fails; old binding still activates; unrelated pause_media persists')
        last = output / 'data/koett/Last Transcript.txt'
        chord('ctrl+alt+space')
        recording()
        subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
        chord('ctrl+alt+space')
        ready()
        wait(lambda: last.exists() and last.read_text().strip(), 'public WAV Last transcript', timeout=60)
        exact_last = last.read_bytes()
        with sqlite3.connect(output / 'data/koett/History.sqlite3') as database:
            latest_text = database.execute('SELECT text FROM transcripts ORDER BY created_at_ms DESC LIMIT 1').fetchone()[0]
        assert exact_last.decode() == latest_text, (exact_last, latest_text)
        proof['last_text'] = latest_text
        proof['last_sha256'] = hashlib.sha256(exact_last).hexdigest()
        checks.append('Public cached WAV through real shortcut capture produces exact matching Last and History')
        settings_page()
        click('Save')
        wait(lambda: not find('Settings', 'frame'), 'Save after transcript')
        assert last.read_bytes() == exact_last, 'Settings Save preserves exact Last'
        checks.append('Settings reopen and Save preserve exact Last bytes')
        control('quit')
        process.wait(timeout=15)
        assert process.returncode == 0
        process = launch()
        assert json.loads(config.read_text())['shortcut'] == 'Ctrl+Alt+Space'
        assert last.read_bytes() == exact_last, 'exact Last preserved across restart'
        control('copy-last')
        copied = subprocess.check_output(['xclip', '-selection', 'clipboard', '-out'])
        assert copied == exact_last, 'Copy Last preserves exact text after restart'
        checks.append('Exact Last bytes survive full restart and Copy Last matches private X11 clipboard')
        toggle('ctrl+alt+space')
        checks.append('Saved swapped binding persists across full process restart and still activates')
        control('quit')
        process.wait(timeout=15)
        assert process.returncode == 0
    proof['pass'] = True
except Exception:
    capture('failure')
    (output / 'accessibility.json').write_text(json.dumps([{'name': n.get_name(), 'role': n.get_role_name()} for n in walk(Atspi.get_desktop(0))], indent=2))
    proof['pass'] = False
    raise
finally:
    (output / 'results.json').write_text(json.dumps(proof, indent=2))
    if process and process.poll() is None:
        process.terminate()
        process.wait(timeout=10)
    connection.close()
    xwayland.terminate()
    xwayland.wait(timeout=10)
