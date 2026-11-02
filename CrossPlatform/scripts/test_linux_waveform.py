#!/usr/bin/env python3
"""Native waveform evidence in owned agent-lab koett-waveform only.

Caller starts/stops the lab. Uses the real release app and CPAL capture path.
All settings, audio, typing, accessibility and screenshots remain private.
"""
import argparse
import ast
import atexit
import json
import math
import os
import struct
import subprocess
import time
import wave
from pathlib import Path

from test_linux_ui import wait

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--binary', type=Path, help='Release executable to verify')
parser.add_argument('--theme', choices=['light', 'dark'], required=True)
parser.add_argument('--reduced', action='store_true')
args = parser.parse_args()
expected = f'/run/user/{os.getuid()}/agent-lab/koett-waveform'
assert os.environ.get('AGENT_LAB_RUNTIME') == expected
assert os.environ.get('XDG_RUNTIME_DIR') == expected
assert os.environ.get('WAYLAND_DISPLAY') == 'wayland-agent-lab'
assert os.environ.get('AGENT_LAB_GENERATION')
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[1]
binary = args.binary.resolve() if args.binary else root / 'target/release/koett'
model = Path.home() / '.local/share/koett/models/sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8'
os.environ.pop('AT_SPI_BUS_ADDRESS', None)
os.environ.pop('NIRI_SOCKET', None)
for kind in ['CONFIG', 'DATA', 'CACHE', 'STATE']:
    os.environ[f'XDG_{kind}_HOME'] = str(output / kind.lower())
os.environ.update(GTK_A11Y='atspi', GTK_THEME='Adwaita:' + args.theme, GTK_USE_PORTAL='0')
config = output / 'config/koett/settings.json'
config.parent.mkdir(parents=True, exist_ok=True)
config.write_text(json.dumps({'model_directory': str(model), 'start_at_login': False}))
(output / 'config/user-dirs.dirs').write_text(f'XDG_DOCUMENTS_DIR="{output}"\n')
gtk_config = output / 'config/gtk-4.0/settings.ini'
gtk_config.parent.mkdir(parents=True, exist_ok=True)
gtk_config.write_text('[Settings]\ngtk-enable-animations=' + ('false' if args.reduced else 'true') + '\n')
# The private desktop portal overrides settings.ini. Exercise its real setting.
subprocess.run(['gsettings', 'set', 'org.gnome.desktop.interface', 'enable-animations',
                'false' if args.reduced else 'true'], check=True)
module = subprocess.check_output(['pactl', 'load-module', 'module-remap-source',
    'master=agent_lab_audio.monitor', 'source_name=waveform_mic'], text=True).strip()
alsa = output / 'alsa.conf'
alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "waveform_mic" }\nctl.!default { type pulse }\n')
os.environ.update(ALSA_CONFIG_PATH=str(alsa), PULSE_SOURCE='waveform_mic')
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


def walk(node):
    yield node
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child:
            yield from walk(child)


def find(name, pid=None):
    return next((node for node in walk(Atspi.get_desktop(0))
                 if node.get_name() == name and (pid is None or node.get_process_id() == pid)
                 and node.get_state_set().contains(Atspi.StateType.SHOWING)), None)


def control(command):
    subprocess.run([str(binary), command], check=True, stdout=subprocess.DEVNULL)


def capture(name):
    path = output / f'{name}.png'
    subprocess.run(['/usr/bin/python3', str(Path.home() / '.local/share/agent-lab/screenshot.py'), str(path)], check=True, stdout=subprocess.DEVNULL)
    image = Image.open(path)
    # The 236x48 layer is bottom centered, with the existing 24px margin.
    x, y = image.width // 2 - 118, image.height - 72
    pill = image.crop((x, y, x + 236, y + 48))
    pill.save(output / f'{name}-pill.png')
    return pill


def heights(image):
    # Bar centers deliberately avoid text/divider/dot and anti-aliased edges.
    return [sum(max(image.getpixel((40 + i * 6, y))[:3]) > 100 for y in range(8, 40)) for i in range(17)]


def audio_file():
    path = output / 'amplitude-steps.wav'
    with wave.open(str(path), 'wb') as audio:
        audio.setparams((1, 2, 48000, 0, 'NONE', 'not compressed'))
        # Alternating measured amplitude steps make a moving history visible.
        audio.writeframes(b''.join(struct.pack('<h', int(32767 * amp * math.sin(2 * math.pi * 440 * n / 48000)))
                          for amp in [0.02, 0.5, 0.04, 0.8] * 3 for n in range(12000)))
    return path


process = target = player = None
checks = []
try:
    log = (output / 'app.log').open('w')
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    wait(lambda: any(n.get_process_id() == process.pid and n.get_name().startswith('Ready') for n in walk(Atspi.get_desktop(0))), 'real model ready', timeout=90)
    target = subprocess.Popen(['/usr/bin/python3', '-c', """
import gi
gi.require_version('Gtk','4.0')
from gi.repository import Gtk
app=Gtk.Application(application_id='com.koett.WaveformTarget')
def activate(app):
    w=Gtk.ApplicationWindow(application=app,title='Waveform typing target')
    entry=Gtk.Entry()
    w.set_child(entry)
    w.maximize()
    w.present()
    entry.grab_focus()
app.connect('activate',activate)
app.run([])
"""])
    owned = wait(lambda: find('Waveform typing target', target.pid), 'owned target')
    wait(lambda: owned.get_state_set().contains(Atspi.StateType.ACTIVE), 'target active')
    input_tool = str(Path.home() / '.local/share/agent-lab/input')
    subprocess.run([input_tool, 'text', 'before '], check=True)
    control('start')
    pill = wait(lambda: find('Koett recording', process.pid), 'mapped native pill')
    time.sleep(0.8)
    flat = heights(capture('silence-start'))
    assert max(flat) <= 4, flat
    assert find('Recording', process.pid)
    player = subprocess.Popen(['pw-play', '--target', 'agent_lab_audio', str(audio_file())])
    frames = []
    measured = []
    for index in range(12):
        frame = capture(f'motion-{index:02}')
        frames.append(frame)
        measured.append(heights(frame))
        time.sleep(0.06)
    player.wait(timeout=10)
    if args.reduced:
        assert all(max(values) <= 4 for values in measured), measured
        checks.append('reduced-motion real audio keeps static flat bars')
    else:
        assert max(map(max, measured)) > 12, measured
        assert len({tuple(values) for values in measured}) > 3, measured
        checks.append('CPAL private amplitude steps produce changing recent waveform')
    frames[0].save(output / 'motion.gif', save_all=True, append_images=frames[1:], duration=120, loop=0)
    if not args.reduced:
        player = subprocess.Popen(['pw-play', '--target', 'agent_lab_audio', str(output / 'amplitude-steps.wav')])
        time.sleep(0.4)
        subprocess.run(['gsettings', 'set', 'org.gnome.desktop.interface', 'enable-animations', 'false'], check=True)
        time.sleep(0.4)
        assert max(heights(capture('dynamic-reduced'))) <= 4
        subprocess.run(['gsettings', 'set', 'org.gnome.desktop.interface', 'enable-animations', 'true'], check=True)
        time.sleep(0.5)
        assert max(heights(capture('dynamic-resumed'))) > 4
        player.wait(timeout=10)
        checks.append('live GTK animation preference clears/freezes bars and resumes real samples')
    assert any(n.get_name().startswith('0:') and n.get_name() != '0:00' for n in walk(pill))
    checks.append('native elapsed timer advances during recording')
    time.sleep(2)
    settled = heights(capture('silence-settled'))
    assert max(settled) <= 4, settled
    subprocess.run([input_tool, 'move', '960', '1032'], check=True)
    subprocess.run([input_tool, 'click'], check=True)
    subprocess.run([input_tool, 'text', 'during'], check=True)
    assert owned.get_state_set().contains(Atspi.StateType.ACTIVE)
    entries = [n for n in walk(owned) if 'EditableText' in n.get_interfaces()]
    assert any('before during' in Atspi.Text.get_text(n, 0, -1) for n in entries)
    assert not pill.get_state_set().contains(Atspi.StateType.ACTIVE)
    checks.append('empty pointer region and recording preserve typing focus')
    control('stop')
    outcome = wait(lambda: 'transcribing' if find('Transcribing…', process.pid) else 'complete' if not find('Koett recording', process.pid) else None, 'stop status')
    if outcome == 'transcribing':
        capture('transcribing')
    wait(lambda: find('Record', process.pid), 'ASR complete', timeout=90)
    wait(lambda: not find('Koett recording', process.pid), 'cleanup')
    control('start')
    wait(lambda: find('Koett recording', process.pid), 'second capture')
    assert max(heights(capture('second-capture'))) <= 4
    time.sleep(1.1)
    control('stop')
    wait(lambda: find('Record', process.pid), 'second completion', timeout=90)
    wait(lambda: not find('Koett recording', process.pid), 'second cleanup')
    checks.extend(['silence settles flat; new capture resets history', f'stop lifecycle: {outcome}; repeated capture cleanup'])
    settings = json.loads(config.read_text())
    settings['microphone'] = 'invalid-private-waveform-input'
    config.write_text(json.dumps(settings))
    control('start')
    wait(lambda: find('Retry', process.pid), 'capture error')
    assert not find('Koett recording', process.pid)
    capture('capture-error')
    control('quit')
    process.wait(timeout=10)
    assert process.returncode == 0
    checks.append('capture failure and quit leave no stale pill')
    (output / 'results.json').write_text(json.dumps({'checks': checks, 'bar_heights': measured}, indent=2))
    print(json.dumps({'checks': checks, 'artifacts': str(output)}))
finally:
    for child in [player, target, process]:
        if child and child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)
    subprocess.run(['pactl', 'unload-module', module], check=True)
