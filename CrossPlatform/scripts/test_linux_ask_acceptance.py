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
parser.add_argument('--speech-missing-key', action='store_true')
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

# Private HTTPS fixture and private Secret Service; no provider traffic.
import http.server
import secrets
import ssl
import threading

fixture = output / 'tls'
fixture.mkdir()
subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(fixture/'key.pem'),'-out',str(fixture/'cert.pem'),'-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost'], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
os.environ['CURL_CA_BUNDLE'] = str(fixture/'cert.pem')
requests = []
first_byte_release = threading.Event()
answer = '**Fixture answer**.\n\n```rust\nlet answer = 42;\n```\n\n| Item | Count |\n| --- | --- |\n| Test | 1 |\n\n$x^2$'
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_): pass
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        assert self.headers['Authorization'] == 'Bearer ' + synthetic_key
        assert len(body['messages']) == 2
        content = body['messages'][1]['content']
        assert len(content) == 2 and content[1]['image_url']['url'].startswith('data:image/jpeg;base64,')
        requests.append({'image': True, 'question': content[0]['text']})
        payload = b''.join(('data: '+json.dumps({'choices':[{'delta':{'content':part}}]})+'\n\n').encode() for part in [answer[:20], answer[20:]]) + (b'data: [DONE]\n\n' if len(requests) == 1 else b'')
        if len(requests) == 3:
            first_byte_release.wait(timeout=10)
        self.send_response(200)
        self.send_header('Content-Length', str(len(payload)))
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for part in [answer[:20], answer[20:]]:
            self.wfile.write(('data: '+json.dumps({'choices':[{'delta':{'content':part}}]})+'\n\n').encode())
            self.wfile.flush()
        if len(requests) == 1: self.wfile.write(b'data: [DONE]\n\n')
server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(fixture/'cert.pem', fixture/'key.pem')
server.socket = context.wrap_socket(server.socket, server_side=True)
threading.Thread(target=server.serve_forever, daemon=True).start()
atexit.register(server.shutdown)
synthetic_key = secrets.token_hex(24)
control_dir = output/'keyring-control'
control_dir.mkdir(mode=0o700)
keyring = subprocess.Popen(['/usr/bin/gnome-keyring-daemon','--foreground','--unlock','--components=secrets','--control-directory='+str(control_dir)], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
keyring.stdin.write(secrets.token_bytes(32).hex().encode())
keyring.stdin.close()
atexit.register(keyring.terminate)
time.sleep(1)
subprocess.run([str(binary),'ask-key','custom'],input=synthetic_key.encode(),check=True,stdout=subprocess.DEVNULL)

config.with_name('assistant.json').write_text(json.dumps({
    'enabled': True, 'provider': 'custom',
    'endpoint': f'https://localhost:{server.server_port}/chat/completions',
    'model': 'qwen/qwen3.6-27b'}))
if args.speech_missing_key:
    config.with_name("speech.json").write_text(json.dumps({"enabled": True, "voice": "Katie"}))
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
    kwin("for (const w of workspace.windowList()) if (w.caption === 'Screen Sharing Requested') { w.frameGeometry = {x:430,y:60,width:740,height:776}; workspace.activeWindow=w; }")
    subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), 'move', '800', '300'], check=True)
    subprocess.run([str(Path.home() / '.local/share/agent-lab/input'), 'click'], check=True)
    panel = wait(lambda: find('Koett Ask', role='frame'), 'Ask panel', timeout=20)
    wait(lambda: find('Listening…', panel), 'listening', timeout=20)
    assert find('Screen included', panel)
    capture('ask-listening')
    subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    control('ask-stop')
    wait(lambda: len(requests) == 1, 'HTTPS request', timeout=30)
    wait(lambda: json.loads(subprocess.check_output([str(binary), 'ask-status']))['phase'] == 'Done', 'full answer', timeout=30)
    button = wait(lambda: find('Copy answer', panel), 'Copy button')
    keys('key', 42)
    assert button.get_action_iface().do_action(0)
    wait(lambda: subprocess.check_output(['wl-paste', '--no-newline']).decode() == answer, 'exact answer Copy')
    if args.speech_missing_key:
        assert find('Spoken reply unavailable; text answer continues', panel)
    capture('ask-full-answer')
    resize('Koett Ask', 480, 580)
    capture('ask-answer-480')
    with sqlite3.connect(output / 'data/koett/History.sqlite3') as connection:
        questions = connection.execute('SELECT text FROM transcripts').fetchall()
    assert len(questions) == 1 and 'old portrait' in questions[0][0].lower(), questions
    status = json.loads(subprocess.check_output([str(binary), 'ask-status']))
    assert status['phase'] == 'Done', status
    assert 'question' not in status and 'answer' not in status
    control('ask-cancel')
    wait(lambda: json.loads(subprocess.check_output([str(binary), 'ask-status']))['phase'] == 'Cancelled', 'cancel acknowledgement')
    time.sleep(0.3)
    control('ask-start')
    time.sleep(2)
    capture('permission-' + str(len(requests)))
    kwin("for (const w of workspace.windowList()) if (w.caption === 'Screen Sharing Requested') { w.frameGeometry = {x:430,y:60,width:740,height:776}; workspace.activeWindow=w; }")
    keys('move', 800, 300)
    keys('click')
    panel = wait(lambda: find('Koett Ask', role='frame'), 'second panel', timeout=20)
    wait(lambda: find('Listening…', panel), 'second question')
    subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    control('ask-stop')
    wait(lambda: find('Could not answer', panel), 'partial disconnect', timeout=30)
    keys('key', 42)
    assert find('Copy answer', panel).get_action_iface().do_action(0)
    wait(lambda: subprocess.check_output(['wl-paste', '--no-newline']).decode() == answer, 'partial exact Copy')
    capture('ask-partial-answer')
    assert len(requests) == 2
    control('ask-cancel')
    wait(lambda: json.loads(subprocess.check_output([str(binary), 'ask-status']))['phase'] == 'Cancelled', 'cancel acknowledgement')
    time.sleep(0.3)
    control('ask-start')
    time.sleep(2)
    capture('permission-' + str(len(requests)))
    kwin("for (const w of workspace.windowList()) if (w.caption === 'Screen Sharing Requested') { w.frameGeometry = {x:430,y:60,width:740,height:776}; workspace.activeWindow=w; }")
    keys('move', 800, 300)
    keys('click')
    panel = wait(lambda: find('Koett Ask', role='frame'), 'third panel', timeout=20)
    wait(lambda: find('Listening…', panel), 'cancellable question')
    subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    control('ask-stop')
    deadline = time.monotonic() + 3
    while True:
        observed = json.loads(subprocess.check_output([str(binary), 'ask-status']))
        if observed['phase'] != 'Listening…' or time.monotonic() > deadline:
            break
    assert observed['phase'] == 'Transcribing locally…', observed
    control('ask-cancel')
    wait(lambda: json.loads(subprocess.check_output([str(binary), 'ask-status']))['phase'] == 'Cancelled', 'ASR cancellation', timeout=30)
    time.sleep(2)
    assert len(requests) == 2, 'provider request after local ASR cancellation'
    capture('ask-cancelled-asr')
    control('ask-start')
    time.sleep(2)
    kwin("for (const w of workspace.windowList()) if (w.caption === 'Screen Sharing Requested') { w.frameGeometry = {x:430,y:60,width:740,height:776}; workspace.activeWindow=w; }")
    keys('move', 800, 300)
    keys('click')
    panel = wait(lambda: find('Koett Ask', role='frame'), 'fourth panel', timeout=20)
    wait(lambda: find('Listening…', panel), 'fourth question')
    subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    control('ask-stop')
    wait(lambda: len(requests) == 3, 'request held before first byte', timeout=30)
    assert not find('Copy answer', panel).get_state_set().contains(Atspi.StateType.SENSITIVE)
    control('ask-cancel')
    wait(lambda: json.loads(subprocess.check_output([str(binary), 'ask-status']))['phase'] == 'Cancelled', 'cancel before first byte')
    first_byte_release.set()
    time.sleep(0.3)
    assert not find('Copy answer', panel).get_state_set().contains(Atspi.StateType.SENSITIVE)
    capture('ask-cancelled-before-first-byte')
    control('ask-start')
    time.sleep(2)
    control('ask-cancel')
    wait(lambda: json.loads(subprocess.check_output([str(binary), 'ask-status']))['phase'] == 'Cancelled', 'cancel pending permission')
    time.sleep(0.3)
    control('ask-start')
    time.sleep(2)
    keys('key', 1)
    wait(lambda: json.loads(subprocess.check_output([str(binary), 'ask-status']))['phase'] == 'Error', 'permission denial', timeout=20)
    assert len(requests) == 3
    capture('ask-permission-denied')
    control('ask-cancel')
    control('start')
    wait(lambda: find('Stop', main), 'dictation after Ask')
    subprocess.run(['pw-play', '--target', 'agent_lab_audio', str(model / 'test_wavs/0.wav')], check=True)
    control('stop')
    wait(lambda: find('Record', main), 'dictation completed', timeout=120)
    with sqlite3.connect(output / 'data/koett/History.sqlite3') as connection:
        rows = connection.execute('SELECT text FROM transcripts').fetchall()
    assert len(rows) == 5 and all('old portrait' in row[0].lower() for row in rows), rows
    capture('dictation-after-ask')
    (output / 'checks.json').write_text(json.dumps({'real_voice_question_saved_once': True, 'private_portal_frame': True, 'private_secret_service_roundtrip': True, 'https_worker_full_answer': True, 'exact_copy': True, 'partial_answer_copy': True, 'cancel_during_observed_real_asr_no_request': True, 'cancel_before_first_byte': True, 'permission_cancel_restart': True, 'permission_denial_no_request': True, 'normal_dictation_after_ask': True, 'cloud_requests': 0, 'local_https_requests': len(requests)}))
finally:
    if process is not None:
        subprocess.run([str(binary), 'quit'], timeout=10, check=False)
        try:
            process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=5)
    registry.terminate()
