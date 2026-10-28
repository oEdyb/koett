#!/usr/bin/env python3
"""Native Settings and recovery evidence; caller owns agent-lab koett-feedback.

Uses only its private compositor, accessibility bus, clipboard, synthetic audio,
settings and history. No app test hooks or model downloads.
"""
import argparse
import ast
import atexit
import json
import math
import os
import sqlite3
import struct
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
parser.add_argument('--settings-only', action='store_true')
args = parser.parse_args()
runtime = f'/run/user/{os.getuid()}/agent-lab/koett-feedback'
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
    'master=agent_lab_audio.monitor', 'source_name=feedback_mic'], text=True).strip()
atexit.register(lambda: subprocess.run(['pactl', 'unload-module', module], check=True))
alsa = output / 'alsa.conf'
alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "feedback_mic" }\nctl.!default { type pulse }\npcm.feedback_input { type pulse device "feedback_mic" hint { show on description "Private feedback input" ioid "Input" } }\n')
os.environ.update(ALSA_CONFIG_PATH=str(alsa), PULSE_SOURCE='feedback_mic')
address = ast.literal_eval(subprocess.check_output(['gdbus', 'call', '--session', '--dest', 'org.a11y.Bus', '--object-path', '/org/a11y/bus', '--method', 'org.a11y.Bus.GetAddress'], text=True).strip())[0]
os.environ['AT_SPI_BUS_ADDRESS'] = address
registry = subprocess.Popen(['/usr/lib/at-spi2-registryd'], stdout=(output / 'accessibility.log').open('w'), stderr=subprocess.STDOUT)
atexit.register(lambda: registry.terminate() if registry.poll() is None else None)
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
    return next((node for node in walk(root or Atspi.get_desktop(0))
                 if node.get_name() == name and (role is None or node.get_role_name() == role) and node.get_state_set().contains(Atspi.StateType.SHOWING)), None)


def click(name, root=None):
    node = wait(lambda: find(name, root), name)
    assert node.get_action_iface().do_action(0), name
    time.sleep(0.2)
    return node


def control(command):
    subprocess.run([str(binary), command], check=True, capture_output=True)


def capture(name):
    time.sleep(0.3)
    subprocess.run(['/usr/bin/python3', str(Path.home() / '.local/share/agent-lab/screenshot.py'), str(output / (name + '.png'))], check=True, stdout=subprocess.DEVNULL)
    print('Captured ' + name, flush=True)


def streams():
    clients = json.loads(subprocess.check_output(['pactl', '-f', 'json', 'list', 'clients']))
    owned = {str(c['index']) for c in clients if str(c.get('properties', {}).get('application.process.id')) == str(process.pid)}
    return [s for s in json.loads(subprocess.check_output(['pactl', '-f', 'json', 'list', 'source-outputs'])) if str(s.get('client')) in owned]


def select(combo, index):
    toggle = next((n for n in walk(combo) if n.get_role_name() == 'toggle button'), None)
    if toggle:
        assert toggle.get_action_iface().do_action(0)
        time.sleep(0.4)
        keys('key', 102, *([108] * index), 28, 15)
    else:
        assert combo.get_selection_iface().select_child(index)
    time.sleep(0.4)


def keys(command, *codes):
    subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), command, *map(str, codes)], check=True)


def kwin(source):
    path = output / 'window.js'
    path.write_text(source)
    ident = subprocess.check_output(['qdbus6', 'org.kde.KWin', '/Scripting', 'loadScript', str(path), 'feedback-test'], text=True).strip()
    try:
        subprocess.run(['qdbus6', 'org.kde.KWin', '/Scripting/Script' + ident, 'org.kde.kwin.Script.run'], check=True, capture_output=True)
    finally:
        subprocess.run(['qdbus6', 'org.kde.KWin', '/Scripting', 'unloadScript', 'feedback-test'], check=True, capture_output=True)
    time.sleep(0.3)


def tone(amplitude, seconds):
    path = output / f'tone-{amplitude}.wav'
    with wave.open(str(path), 'wb') as audio:
        audio.setparams((1, 2, 48000, 0, 'NONE', 'not compressed'))
        audio.writeframes(b''.join(struct.pack('<h', int(32767 * amplitude * math.sin(2 * math.pi * 440 * n / 48000))) for n in range(int(48000 * seconds))))
    return subprocess.Popen(['pw-play', '--target=agent_lab_audio', str(path)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def rows():
    db = output / 'data/koett/History.sqlite3'
    if not db.exists():
        return []
    with sqlite3.connect(db) as connection:
        return connection.execute('SELECT text FROM transcripts ORDER BY created_at_ms').fetchall()


helpers = output / 'helpers'
helpers.mkdir()
copy_failure = output / 'force-copy-failure'
wrapper = helpers / 'wl-copy'
wrapper.write_text('#!/usr/bin/python3\nimport os,sys\nfrom pathlib import Path\nif Path(' + repr(str(copy_failure)) + ').exists(): sys.exit(1)\nos.execv("/usr/bin/wl-copy", ["wl-copy", *sys.argv[1:]])\n')
wrapper.chmod(0o700)
app_env = {**os.environ, 'PATH': str(helpers) + ':' + os.environ['PATH']}
process = target = player = None
checks = []
try:
    log = (output / 'app.log').open('w')
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log, env=app_env)
    wait(lambda: find('Ready'), 'real model ready', timeout=120)
    click('Settings')
    settings = wait(lambda: find('Settings', role='frame'), 'Settings window')
    wait(lambda: len(streams()) == 1, 'Audio preview initially captures')
    assert find('Microphone', settings)
    # Native dialog geometry remains narrow; inspect every section in both themes.
    for section in ['Audio', 'Shortcuts', 'Text', 'General']:
        click(section, settings)
        assert find('Save', settings)
        if section != 'Audio':
            wait(lambda: not streams(), 'hidden Audio releases microphone')
        capture('settings-' + section.lower())
    checks.append('all four native sections and persistent Save at default 460px; hidden Audio releases capture')
    if args.settings_only:
        checks.append('Settings render only; renderer=' + args.renderer)
        (output / 'results.json').write_text(json.dumps(checks, indent=2))
        raise SystemExit(0)
    # Real keyboard navigation across the tablist.
    tab = find('Audio', settings)
    rect = tab.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
    keys('move', rect.x + rect.width // 2, rect.y + rect.height // 2)
    keys('click')
    keys('key', 106, 57)
    wait(lambda: find('Recording mode', settings), 'keyboard selects Shortcuts')
    checks.append('native tab keyboard access')
    click('Text', settings)
    select(wait(lambda: find('Paste shortcut', settings, 'combo box'), 'paste control'), 1)
    click('Add replacement', settings)
    phrase = wait(lambda: find('Phrase', settings, 'text'), 'phrase entry')
    replacement = wait(lambda: find('Replacement', settings, 'text'), 'replacement entry')
    assert phrase.get_editable_text_iface().set_text_contents('feedbackfixture')
    assert replacement.get_editable_text_iface().set_text_contents('Exact Åäö 日本語')
    click('General', settings)
    click('Save', settings)
    wait(lambda: not find('Settings', role='frame'), 'Save closes Settings')
    saved = json.loads(config.read_text())
    assert saved['paste_shortcut'] == 'ctrl_shift_v'
    assert saved['replacements'] == [{'phrase': 'feedbackfixture', 'replacement': 'Exact Åäö 日本語'}]
    control('settings')
    settings = wait(lambda: find('Settings', role='frame'), 'reopen Settings')
    assert find('Microphone', settings), 'Audio initially selected again'
    wait(lambda: len(streams()) == 1, 'preview reopened')
    # Device selection restarts the actual preview, and never saves implicitly.
    select(wait(lambda: find('Microphone', settings, 'combo box'), 'microphone control'), 1)
    wait(lambda: not streams(), 'null selection releases preview')
    select(wait(lambda: find('Microphone', settings, 'combo box'), 'microphone control'), 2)
    wait(lambda: len(streams()) == 1, 'selected device preview')
    assert json.loads(config.read_text())['microphone'] is None
    click('Text', settings)
    wait(lambda: (entry := find('Replacement', settings, 'text')) and Atspi.Text.get_text(entry, 0, -1) == 'Exact Åäö 日本語', 'reopened exact replacement')
    click('Add replacement', settings)
    invalid_phrase = wait(lambda: next((n for n in walk(settings)
        if n.get_name() == 'Phrase' and 'EditableText' in n.get_interfaces()
        and Atspi.Text.get_text(n, 0, -1) == ''), None), 'new empty Phrase entry')
    click('General', settings)
    click('Save', settings)
    wait(lambda: find('Enter a phrase or remove the empty replacement.', settings), 'invalid replacement error')
    assert find('Word replacements', settings), 'validation selects Text'
    assert invalid_phrase.get_state_set().contains(Atspi.StateType.FOCUSED), 'invalid Phrase receives focus'
    capture('invalid-selects-text')
    checks.append('paste shortcut and exact replacement persist; invalid Save selects Text and focuses phrase')
    # Close without Save discards empty draft and releases microphone.
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Settings") w.closeWindow();')
    wait(lambda: not find('Settings', role='frame'), 'close Settings')
    wait(lambda: not streams(), 'closed preview releases capture')
    control('settings')
    settings = wait(lambda: find('Settings', role='frame'), 'Audio warning preview')
    wait(lambda: len(streams()) == 1, 'preview capture running')
    time.sleep(6)
    assert not find('No input detected', settings), 'thinking silence is not a preview error'
    capture('preview-no-input')
    player = tone(1.0, 3)
    wait(lambda: find('Input too loud', settings), 'debounced preview loud input', timeout=8)
    capture('preview-too-loud')
    player.wait(timeout=8)
    checks.append('real private CPAL preview accepts silence and warns only on sustained loud input')
    # Preview must yield to actual capture, even while Audio stays selected.
    control('start')
    wait(lambda: find('Preview paused while recording', settings), 'preview yields')
    wait(lambda: len(streams()) == 1, 'sole stream belongs to dictation')
    recording_pill = wait(lambda: find('Koett recording'), 'recording pill')
    time.sleep(6)
    assert find('Stop'), 'silent capture stays recording'
    assert not find('No input detected', recording_pill), 'thinking silence never warns'
    capture('capture-no-input')
    player = tone(1.0, 3)
    wait(lambda: find('Input too loud', recording_pill), 'pill loud warning', timeout=8)
    capture('capture-too-loud')
    player.wait(timeout=8)
    control('stop')
    wait(lambda: find('Record'), 'silence/tone capture finished', timeout=90)
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Settings") w.closeWindow();')
    wait(lambda: not streams(), 'preview close after dictation')
    checks.append('recording uses one real stream, preview yields; real pill warnings clear across capture lifecycle')
    # Guarded delivery: begin with Koett focused, then finish while owned target
    # has focus. This forces clipboard-only without OS permission changes.
    gui = output / 'target.py'
    gui.write_text('''import gi
gi.require_version('Gtk','4.0')
from gi.repository import Gtk
app=Gtk.Application(application_id='com.koett.FeedbackTarget')
def activate(app):
 w=Gtk.ApplicationWindow(application=app,title='Feedback typing target');w.set_default_size(600,300)
 e=Gtk.Entry();w.set_child(e);w.present();e.grab_focus()
app.connect('activate',activate);app.run([])
''')
    target = subprocess.Popen(['/usr/bin/python3', str(gui)], stdout=(output / 'target.log').open('w'), stderr=subprocess.STDOUT)
    target_window = wait(lambda: find('Feedback typing target'), 'target')
    for run in range(2):
        if run == 1:
            copy_failure.touch()
        kwin('for (const w of workspace.stackingOrder) if (w.caption === "Koett") workspace.activeWindow=w;')
        count = len(rows())
        control('start')
        wait(lambda: find('Stop'), 'new capture')
        assert not find('Copy transcript'), 'fresh capture clears recovery'
        kwin('for (const w of workspace.stackingOrder) if (w.caption === "Feedback typing target") workspace.activeWindow=w;')
        if run == 0:
            kwin('for (const w of workspace.stackingOrder) if (w.caption === "Feedback typing target") w.frameGeometry={x:0,y:0,width:1920,height:1080};')
            keys('move', 960, 1032)
            keys('click')
            keys('text', 'during capture')
            entry = next(n for n in walk(target_window) if 'EditableText' in n.get_interfaces())
            wait(lambda entry=entry: Atspi.Text.get_text(entry, 0, -1) == 'during capture', 'pill pointer pass-through and typing')
        subprocess.run(['pw-play', '--target=agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
        control('stop')
        wait(lambda count=count: len(rows()) > count, 'real ASR result persisted', timeout=120)
        pill = wait(lambda: find('Koett recording'), 'recovery pill')
        wait(lambda pill=pill: find('Copy transcript', pill), 'explicit recovery button')
        assert target_window.get_state_set().contains(Atspi.StateType.ACTIVE), 'recovery must not activate Koett'
        if run == 0:
            # Pointer action, not accessibility invoke: proves recovery hit region.
            button = find('Copy transcript', pill)
            from PIL import Image
            capture('delivery-recovery-before-copy')
            image = Image.open(output / 'delivery-recovery-before-copy.png')
            # Wayland does not expose global surface positions through AT-SPI.
            # Layer-shell anchors this 236px surface 24px above the bottom.
            keys('move', image.width // 2, image.height - 48)
            keys('click')
            wait(lambda pill=pill: find('Copied', pill), 'copy confirmed only after clipboard set')
            actual = subprocess.check_output(['wl-paste', '--no-newline']).decode()
            assert actual == rows()[-1][0]
            assert target_window.get_state_set().contains(Atspi.StateType.ACTIVE)
            capture('delivery-recovery-copied')
        else:
            click('Copy transcript', pill)
            wait(lambda pill=pill: find('Copied', pill), 'clipboard failure recovery copied')
            assert subprocess.check_output(['wl-paste', '--no-newline']).decode() == rows()[-1][0]
            assert target_window.get_state_set().contains(Atspi.StateType.ACTIVE)
            capture('delivery-recovery')
            wait(lambda: not find('Koett recording'), 'recovery timeout hides pill', timeout=25)
            assert find('Copy last transcript'), 'last-copy survives transient timeout'
            wait(lambda: find('Ready'), 'notices expire back to Ready', timeout=12)
            capture('recovery-expired')
            subprocess.run(['/usr/bin/wl-copy', '--type', 'text/plain;charset=utf-8'], input=b'private replacement sentinel', check=True)
            kwin('for (const w of workspace.stackingOrder) if (w.caption === "Koett") workspace.activeWindow=w;')
            keys('key', 15)
            click('Copy last transcript')
            wait(lambda: subprocess.check_output(['wl-paste', '--no-newline']).decode() == rows()[-1][0], 'last-copy exact after expiry')
    checks.append('guarded clipboard-only and controlled external clipboard failure retain exact ASR text; recovery Copy matches SQLite bytes without focus change')
    checks.append('next capture clears recovery; 20-second timeout hides pill, retains Copy last, and restores Ready')
    kwin('for (const w of workspace.stackingOrder) if (w.caption === "Koett") { w.frameGeometry = {x:300,y:100,width:460,height:600}; workspace.activeWindow=w; }')
    capture('main-narrow')
    (output / 'results.json').write_text(json.dumps(checks, indent=2))
    print(json.dumps({'checks': checks, 'evidence': str(output)}, indent=2))
except Exception:
    capture('failure')
    (output / 'accessibility.json').write_text(json.dumps([{'name': n.get_name(), 'role': n.get_role_name(), 'states': str(n.get_state_set().get_states())} for n in walk(Atspi.get_desktop(0))], indent=2))
    raise
finally:
    for child in [player, target]:
        if child is not None and child.poll() is None:
            child.terminate()
            child.wait(timeout=5)
    if process is not None and process.poll() is None:
        control('quit')
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=5)
