#!/usr/bin/env python3
"""Exercise packaged formatting and raw recovery through real private capture."""
import argparse
import hashlib
import json
import os
import shutil
import signal
import sqlite3
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--model', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--lab', required=True)
args = parser.parse_args()
assert os.environ['AGENT_LAB_RUNTIME'] == f'/run/user/{os.getuid()}/agent-lab/{args.lab}'
assert os.environ['WAYLAND_DISPLAY'] == 'wayland-agent-lab'
assert os.environ['AGENT_LAB_GENERATION']
for variable in ['NIRI_SOCKET', 'XDG_ACTIVATION_TOKEN', 'DESKTOP_STARTUP_ID', 'AT_SPI_BUS_ADDRESS']:
    os.environ.pop(variable, None)
args.output.mkdir(parents=True, exist_ok=False)
for kind in ['CONFIG', 'DATA', 'CACHE', 'STATE']:
    os.environ[f'XDG_{kind}_HOME'] = str(args.output / kind.lower())
manifest = json.loads((Path(__file__).resolve().parents[1] / 'model-manifests/parakeet-tdt-ctc-110m-int8.json').read_text())
source = Path.home() / '.local/share/koett/models' / manifest['model_id']
model = args.output / 'models' / manifest['model_id']
for item in manifest['files']:
    path = source / item['path']
    assert hashlib.sha256(path.read_bytes()).hexdigest() == item['sha256']
    destination = model / item['path']
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, destination)
with args.model.open('rb') as stream:
    assert hashlib.file_digest(stream, 'sha256').hexdigest() == '3b41ebe2502cbd03e811d5d16b022f5ab551eda58d62597d152f89535003c634'
format_model = args.output / 'cache/koett/S1-mini/s1-mini-q4_k_m.gguf'
format_model.parent.mkdir(parents=True)
shutil.copy2(args.model, format_model)
config = args.output / 'config/koett/settings.json'
config.parent.mkdir(parents=True)
config.write_text(json.dumps({'start_at_login': False, 'model_directory': str(model)}))
alsa = args.output / 'alsa.conf'
alsa.write_text('</usr/share/alsa/alsa.conf>\npcm.!default { type pulse device "package_format_mic" }\nctl.!default { type pulse }\n')
os.environ.update(ALSA_CONFIG_PATH=str(alsa), PULSE_SOURCE='package_format_mic', GTK_USE_PORTAL='0', GTK_A11Y='none')
module = subprocess.check_output(['pactl', 'load-module', 'module-remap-source', 'master=agent_lab_audio.monitor', 'source_name=package_format_mic'], text=True).strip()
database = args.output / 'data/koett/History.sqlite3'


def wait(predicate, label, timeout=90):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            value = predicate()
            if value:
                return value
        except (subprocess.SubprocessError, ValueError, sqlite3.OperationalError, FileNotFoundError):
            pass
        assert child.poll() is None, label
        time.sleep(0.15)
    raise AssertionError(label)


def request(*command):
    return json.loads(subprocess.check_output([str(args.binary), *command], text=True, timeout=8))


def rows():
    with sqlite3.connect(database) as connection:
        return connection.execute('SELECT text,raw_text FROM transcripts ORDER BY created_at_ms,id').fetchall()


def streams():
    return [item for item in json.loads(subprocess.check_output(['pactl', '-f', 'json', 'list', 'source-outputs'], text=True)) if item.get('properties', {}).get('node.name') == 'alsa_capture.koett']


def record(expected):
    subprocess.run([str(args.binary), 'start'], check=True)
    wait(streams, 'private CPAL capture')
    subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    subprocess.run([str(args.binary), 'stop'], check=True)
    return wait(lambda: rows() if len(rows()) == expected else None, 'saved transcription', timeout=180)


child = None
try:
    with (args.output / 'app.log').open('w') as log:
        child = subprocess.Popen([str(args.binary), '--background'], stdout=log, stderr=log)
        wait(lambda: request('menu-status').get('active_engine'), 'warm recognizer')
        assert request('menu-format', 's1-mini')['ok']
        wait(lambda: json.loads(config.read_text()).get('formatting', {}).get('enabled'), 'product S1-mini preparation', timeout=120)
        first = record(1)
        assert first[0][0] and first[0][1] and 'phoebe' in first[0][1].lower(), first
        server = subprocess.check_output(['pgrep', '-P', str(child.pid), '-x', 'llama-server'], text=True).split()
        assert len(server) == 1, server
        os.kill(int(server[0]), signal.SIGKILL)
        second = record(2)
        assert second[1][1] is None and second[1][0] == first[0][1], second
        assert request('menu-format', 'raw')['ok']
        wait(lambda: not json.loads(config.read_text()).get('formatting', {}).get('enabled'), 'Raw preference saved')
        assert request('copy-last')['ok']
        assert subprocess.check_output(['wl-paste', '--no-newline'], text=True) == second[1][0]
        subprocess.run(['wl-copy', '--clear'], check=True)
        assert not streams()
        assert request('quit')['ok']
        assert child.wait(timeout=15) == 0
        (args.output / 'results.json').write_text(json.dumps({'packaged_launcher':str(args.binary), 'product_s1_capture':True, 'raw_final_retained':True, 'formatter_failure_exact_raw':True, 'history_rows':len(second), 'raw_preference_saved':True, 'copy_last_exact':True, 'zero_capture_streams':True, 'rows':second}, ensure_ascii=False, indent=2))
finally:
    if child is not None and child.poll() is None:
        child.terminate()
        child.wait(timeout=10)
    subprocess.run(['pactl', 'unload-module', module], check=True)
