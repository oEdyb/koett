#!/usr/bin/env python3
"""Actual GTK visual matrix with synthetic text in a private agent-lab session."""
import argparse
import ast
import atexit
import json
import os
import shutil
import signal
import subprocess
import time
from pathlib import Path

from test_linux_ui import wait

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--lab', default='koett-browser-media')
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--theme', choices=['light', 'dark', 'highcontrast'], required=True)
parser.add_argument('--scaled', action='store_true')
parser.add_argument('--visual-only', action='store_true')
parser.add_argument('--renderer', choices=['default', 'cairo', 'gl'], default='default')
parser.add_argument('--media-format', choices=['wav', 'hls', 'dash'], default='wav')
args = parser.parse_args()
runtime = f'/run/user/{os.getuid()}/agent-lab/{args.lab}'
assert os.environ.get('AGENT_LAB_RUNTIME') == runtime
assert os.environ.get('XDG_RUNTIME_DIR') == runtime
assert os.environ.get('WAYLAND_DISPLAY') == 'wayland-agent-lab'
assert os.environ.get('AGENT_LAB_GENERATION')
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
binary = args.binary.resolve()
lab_data = Path(os.environ['XDG_DATA_HOME'])
assert lab_data == Path.home() / '.local/state/agent-lab' / args.lab / 'data'
desktop = lab_data / 'applications/com.olledyberg.Koett.desktop'
desktop.parent.mkdir(parents=True, exist_ok=True)
desktop.write_text(f'[Desktop Entry]\nType=Application\nName=Koett\nExec={binary}\nTerminal=false\n')
os.environ.pop('NIRI_SOCKET', None)
os.environ.pop('XDG_ACTIVATION_TOKEN', None)
os.environ.pop('DESKTOP_STARTUP_ID', None)
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
fixture_model.mkdir(parents=True)
manifest = json.loads((Path(__file__).resolve().parents[1] / 'model-manifests/parakeet-tdt-ctc-110m-int8.json').read_text())
for item in manifest['files']:
    destination = fixture_model / item['path']
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(model / item['path'], destination)
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


def desktop_binding(codes):
    action = ['com.olledyberg.Koett', 'media', 'Koett', 'Transcribe media']
    base = ['gdbus', 'call', '--session', '--dest', 'org.kde.kglobalaccel',
            '--object-path', '/kglobalaccel', '--method']
    subprocess.run([*base, 'org.kde.KGlobalAccel.doRegister', repr(action)], check=True, capture_output=True)
    subprocess.run([*base, 'org.kde.KGlobalAccel.setForeignShortcut', repr(action), repr(codes)], check=True, capture_output=True)


def dismiss_manual():
    dialog = wait(lambda: find('Transcribe media', role='frame'), 'manual URL dialog')
    find('Cancel', dialog).get_action_iface().do_action(0)
    wait(lambda: not find('Media URL'), 'manual dialog closes')


def focus_browser(entry):
    kwin(f'for (const w of workspace.windowList()) if (w.pid === {browser.pid}) workspace.activeWindow=w;')
    assert entry.get_component_iface().grab_focus()
    frame = entry
    while frame and frame.get_role_name() != 'frame':
        frame = frame.get_parent()
    assert frame and frame.get_state_set().contains(Atspi.StateType.ACTIVE)


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



import hashlib
import http.server
import threading

fixture = output / 'fixture'
fixture.mkdir()
(fixture / 'public.wav').symlink_to(model / 'test_wavs/0.wav')
(fixture / 'slow.wav').symlink_to(model / 'test_wavs/0.wav')
media_source = 'public.wav'
if args.media_format != 'wav':
    media_source = 'finite.m3u8' if args.media_format == 'hls' else 'finite.mpd'
    segment_options = ['-f', 'hls', '-hls_time', '2', '-hls_playlist_type', 'vod'] if args.media_format == 'hls' else ['-f', 'dash', '-seg_duration', '2']
    subprocess.run(['/usr/bin/ffmpeg', '-v', 'error', '-i', str(fixture / 'public.wav'),
                    '-vn', '-c:a', 'aac', '-b:a', '96k', *segment_options, str(fixture / media_source)], check=True)
(fixture / 'index.html').write_text(f'<!doctype html><title>Private media fixture</title><input autofocus aria-label="Private typing"><audio controls src="{media_source}"></audio>')
class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=str(fixture), **k)
    def do_GET(self):
        if self.path == '/slow.wav':
            time.sleep(4)
        try:
            super().do_GET()
        except (BrokenPipeError, ConnectionResetError):
            pass
server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
url = f'http://127.0.0.1:{server.server_port}/index.html'
process = browser = None
desktop_binding([])
try:
    log = (output / 'app.log').open('w')
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    main = wait(lambda: find('Koett', role='frame'), 'main')
    wait(lambda: any(n.get_role_name() == 'label' and n.get_name().startswith('Ready') for n in walk(main)), 'ready', timeout=120)
    libraries = {line.split()[-1] for line in Path(f'/proc/{process.pid}/maps').read_text().splitlines() if 'libgtk-4.so.' in line}
    assert len(libraries) == 1, libraries
    gtk_library = Path(libraries.pop()).resolve(strict=True)
    (output / 'runtime.json').write_text(json.dumps({'gtk_library':str(gtk_library), 'gtk_sha256':hashlib.sha256(gtk_library.read_bytes()).hexdigest()}))
    browser = subprocess.Popen(['google-chrome-stable', '--user-data-dir=' + str(output / 'browser-profile'), '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--force-renderer-accessibility=complete', '--ozone-platform=wayland', '--disable-features=Vulkan', '--disable-gpu', url], stdout=(output / 'browser.log').open('w'), stderr=subprocess.STDOUT)
    entry = wait(lambda: find('Private typing'), 'browser input', timeout=30)
    assert entry.get_component_iface().grab_focus()
    def request(command):
        return json.loads(subprocess.check_output([str(binary), command], text=True))
    started = request('media-current')
    (output / 'start.json').write_text(json.dumps(started))
    assert started['ok'], started
    assert request('media-current')['error']['code'] == 'busy'
    keys('text', 'retained focus')
    assert Atspi.Text.get_text(entry, 0, -1) == 'retained focus'
    wait(lambda: request('media-status').get('ok'), 'media result', timeout=120)
    saved = next((output / 'data/koett/Media Transcripts').glob('*.md'))
    assert request('media-copy')['ok']
    clipboard = subprocess.check_output(['wl-paste', '--no-newline'], text=True)
    assert clipboard and clipboard == saved.read_text().split('\n\n', 1)[1]
    result = wait(lambda: find('Copy'), 'result pill Copy')
    assert result is not None, 'result Copy pill missing'
    result.get_action_iface().do_action(0)
    assert subprocess.check_output(['wl-paste', '--no-newline'], text=True) == clipboard
    capture('result-copy')
    count = len(list((output / 'data/koett/Media Transcripts').glob('*.md')))
    (fixture / 'second.html').write_text(f'<!doctype html><title>Second private tab</title><input autofocus aria-label="Second typing"><audio controls src="{media_source}"></audio>')
    keys('chord', 29, 38)
    keys('text', f'http://127.0.0.1:{server.server_port}/second.html')
    keys('key', 28)
    second = wait(lambda: find('Second typing'), 'second document')
    assert second.get_component_iface().grab_focus()
    subprocess.run([str(binary), 'media', f'http://127.0.0.1:{server.server_port}/slow.wav'], check=True)
    time.sleep(0.5)
    control('cancel-media')
    time.sleep(1)
    assert len(list((output / 'data/koett/Media Transcripts').glob('*.md'))) == count
    assert request('media-current')['ok']
    wait(lambda: len(list((output / 'data/koett/Media Transcripts').glob('*.md'))) > count, 'repeated result', timeout=120)
    assert any('/second.html' in p.read_text() for p in (output / 'data/koett/Media Transcripts').glob('*.md'))
    keys('text', 'still typing')
    assert Atspi.Text.get_text(second, 0, -1) == 'still typing'
    time.sleep(11)
    assert not find('Copy'), 'result pill did not expire'
    assert request('media-copy')['ok'], 'cache disappeared with pill'
    desktop_binding([100663380])
    (output / 'shortcut-state.json').write_text(json.dumps(request('shortcuts-status')))
    wait(lambda: 'media' in request('shortcuts-status').get('active', []), 'live media shortcut')
    assert second.get_component_iface().grab_focus()
    before_shortcut = len(list((output / 'data/koett/Media Transcripts').glob('*.md')))
    keys('chord', 29, 42, 20)
    wait(lambda: len(list((output / 'data/koett/Media Transcripts').glob('*.md'))) > before_shortcut, 'global shortcut browser ASR', timeout=120)
    keys('text', ' after shortcut')
    assert Atspi.Text.get_text(second, 0, -1) == 'still typing after shortcut'
    capture('global-shortcut-result')
    from concurrent.futures import ThreadPoolExecutor
    browser.send_signal(signal.SIGSTOP)
    try:
        with ThreadPoolExecutor(max_workers=1) as pool:
            future = pool.submit(request, 'media-current')
            time.sleep(0.1)
            kwin(f'for (const w of workspace.windowList()) if (w.pid === {process.pid} && w.caption === "Koett") workspace.activeWindow=w;')
            browser.send_signal(signal.SIGCONT)
            changed = future.result(timeout=4)
        assert changed['error']['code'] == 'manual_url_required', changed
        assert changed['error']['message'] == 'Focus or document changed; invoke media again', changed
        (output / 'focus-change.json').write_text(json.dumps(changed))
        assert len(list((output / 'data/koett/Media Transcripts').glob('*.md'))) == before_shortcut + 1
    finally:
        browser.send_signal(signal.SIGCONT)
    capture('focus-change-rejected')
    dismiss_manual()
    focus_browser(second)
    keys('chord', 29, 20)
    keys('text', url)
    keys('key', 28)
    wait(lambda: find('Private typing'), 'second browser tab ready')
    keys('chord', 29, 42, 15)
    second = wait(lambda: find('Second typing'), 'first browser tab restored')
    focus_browser(second)
    browser.send_signal(signal.SIGSTOP)
    try:
        with ThreadPoolExecutor(max_workers=1) as pool:
            future = pool.submit(request, 'media-current')
            time.sleep(0.1)
            keys('chord', 29, 15)
            browser.send_signal(signal.SIGCONT)
            changed_tab = future.result(timeout=4)
        assert changed_tab['error']['code'] == 'manual_url_required', changed_tab
        assert changed_tab['error']['message'] == 'Focus or document changed; invoke media again', changed_tab
        (output / 'document-change.json').write_text(json.dumps(changed_tab))
        assert len(list((output / 'data/koett/Media Transcripts').glob('*.md'))) == before_shortcut + 1
    finally:
        browser.send_signal(signal.SIGCONT)
    capture('document-change-rejected')
    dismiss_manual()
    entry = wait(lambda: find('Private typing'), 'original document after rejected navigation')
    focus_browser(entry)
    browser.send_signal(signal.SIGSTOP)
    try:
        began = time.monotonic()
        timed_out = request('media-current')
        assert timed_out['error']['code'] == 'manual_url_required', timed_out
        assert time.monotonic() - began < 3
    finally:
        browser.send_signal(signal.SIGCONT)
    capture('accessibility-timeout')
    keys('key', 1)
    browser.terminate()
    browser.wait(timeout=10)
    browser = None
    fallback = request('media-current')
    assert fallback['error']['code'] == 'manual_url_required', fallback
    wait(lambda: find('Media URL'), 'manual fallback')
    capture('manual-fallback')
    (output / 'receipt.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(), 'media_format':args.media_format, 'clipboard_exact_saved': True, 'ui_cli_copy_equal':True, 'typing_retained':True, 'busy_rejected':True, 'cancel_retry':True, 'global_media_shortcut':True, 'focus_change_rejected':True, 'midlookup_navigation_rejected':True, 'unresponsive_browser_bounded':True, 'current_document_changed':True, 'repeated_completion':True, 'timer_expired_cache_retained':True, 'nonbrowser_manual_fallback':True, 'browser':'google-chrome-stable'}, indent=2))
finally:
    if browser:
        browser.terminate()
        browser.wait(timeout=10)
    if process:
        control('quit')
        process.wait(timeout=10)
    server.shutdown()
