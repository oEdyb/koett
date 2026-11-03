#!/usr/bin/env python3
"""Capture every Linux Settings page, the header menu and the tray menu in a private agent-lab desktop."""
import argparse
import ast
import atexit
import json
import os
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--theme', choices=['light', 'dark'], required=True)
parser.add_argument('--layout', choices=['combo', 'sidebar'], required=True)
args = parser.parse_args()
runtime = os.environ.get('AGENT_LAB_RUNTIME', '')
assert runtime and runtime == os.environ.get('XDG_RUNTIME_DIR'), 'run inside agent-lab'
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
binary = args.binary.resolve()
os.environ.pop('NIRI_SOCKET', None)
for kind in ['CONFIG', 'DATA', 'CACHE', 'STATE']:
    os.environ[f'XDG_{kind}_HOME'] = str(output / 'home' / kind.lower())
os.environ.update(GTK_A11Y='atspi', GTK_USE_PORTAL='0', GTK_THEME='Adwaita:' + args.theme)
config = output / 'home/config/koett/settings.json'
config.parent.mkdir(parents=True)
model = Path.home() / '.local/share/koett/models/sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8'
config.write_text(json.dumps({
    'model_directory': str(model), 'start_at_login': False, 'draw_while_dictating': True,
    'replacements': [{'phrase': 'koet', 'replacement': 'Koett'}],
    'dictionary': [{'term': 'Koett', 'sounds_like': ['cot', 'kett']}],
}))
address = ast.literal_eval(subprocess.check_output(['gdbus', 'call', '--session', '--dest', 'org.a11y.Bus',
    '--object-path', '/org/a11y/bus', '--method', 'org.a11y.Bus.GetAddress'], text=True).strip())[0]
os.environ['AT_SPI_BUS_ADDRESS'] = address
registry = subprocess.Popen(['/usr/lib/at-spi2-registryd'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
atexit.register(registry.terminate)
subprocess.run(['gdbus', 'call', '--session', '--dest', 'org.a11y.Bus', '--object-path', '/org/a11y/bus',
    '--method', 'org.freedesktop.DBus.Properties.Set', 'org.a11y.Status', 'IsEnabled', '<true>'],
    check=True, stdout=subprocess.DEVNULL)
import gi

gi.require_version('Atspi', '2.0')
from gi.repository import Atspi

process = subprocess.Popen([str(binary)], stdout=(output / 'koett.log').open('w'), stderr=subprocess.STDOUT)
atexit.register(lambda: process.terminate() if process.poll() is None else None)


def walk(node):
    yield node
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child:
            yield from walk(child)


def find(name, root=None, role=None):
    return next((n for n in walk(root or Atspi.get_desktop(0)) if n.get_name() == name
                 and (role is None or n.get_role_name() == role)
                 and n.get_state_set().contains(Atspi.StateType.SHOWING)), None)


def wait(predicate, label, timeout=30):
    deadline = time.time() + timeout
    while time.time() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.2)
    raise TimeoutError(label)


def act(node):
    target = next(n for n in walk(node) if n.get_action_iface() and n.get_action_iface().get_n_actions())
    target.get_action_iface().do_action(0)


def capture(name):
    time.sleep(0.6)
    subprocess.run(['/usr/bin/python3', str(Path.home() / '.local/share/agent-lab/screenshot.py'),
                    str(output / (name + '.png'))], check=True, stdout=subprocess.DEVNULL)
    print('Captured', name, flush=True)


def keys(*codes):
    for code in codes:
        subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), 'key', str(code)], check=True)
        time.sleep(0.15)


def control(*command):
    return subprocess.run([str(binary), *command], capture_output=True, text=True, timeout=10)


main = wait(lambda: find('Koett', role='frame'), 'main window', 60)
time.sleep(2)
menu = wait(lambda: find('Menu', main), 'header Menu')
act(menu)
capture('main-menu')
keys(1)
control('settings')
settings = wait(lambda: find('Settings', role='frame'), 'Settings')
capture('settings-open')
if args.layout == 'combo':
    combo = wait(lambda: find('Settings section', settings), 'section selector')
    act(combo)
    capture('settings-sections')
    keys(1)
    for index in range(1, 6):
        act(combo)
        time.sleep(0.4)
        keys(108, 28)
        capture(f'settings-page-{index}')
else:
    pages = ['Dictation', 'Model & formatting', 'Draw on screen', 'Shortcuts', 'Words', 'Ask About Screen', 'General']
    sidebar = wait(lambda: next((n for n in walk(settings) if n.get_role_name() == 'list'
                                 and any(c.get_name() == pages[0] or any(g.get_name() == pages[0] for g in walk(c))
                                         for c in (n.get_child_at_index(i) for i in range(n.get_child_count())))), None),
                   'sidebar')
    for index, page in enumerate(pages):
        assert sidebar.get_selection_iface().select_child(index), page
        capture('settings-' + page.lower().replace(' & ', '-').replace(' ', '-'))
    control('menu-open', 'models')
    capture('settings-menu-open-models')
print(json.dumps({'ok': True, 'output': str(output)}))
