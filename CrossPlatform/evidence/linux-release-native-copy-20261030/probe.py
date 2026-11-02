import sys
from pathlib import Path

sys.path.insert(0, str(Path.cwd() / 'CrossPlatform/scripts'))
source = Path.cwd() / 'CrossPlatform/scripts/test_linux_models.py'
ns = {'__file__': str(source), '__name__': '__main__'}
exec(compile(source.read_text().split('process = None\nchecks = []\ntry:')[0], str(source), 'exec'), ns)  # noqa: S102 - existing private fixture bootstrap
import hashlib
import json
import shutil
import subprocess
import time

os = ns['os']
os.environ.pop('XDG_ACTIVATION_TOKEN', None)
os.environ.pop('DESKTOP_STARTUP_ID', None)
model, fixture = ns['model'], ns['fixture_model']
fixture.unlink()
manifest = json.loads((source.parents[1] / 'model-manifests/parakeet-tdt-ctc-110m-int8.json').read_text())
for item in manifest['files']:
    original = model / item['path']
    assert hashlib.sha256(original.read_bytes()).hexdigest() == item['sha256']
    dest = fixture / item['path']
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(original, dest)
if os.environ.get('COPY_V3'):
    v3_manifest = json.loads((source.parents[1] / 'model-manifests/parakeet-tdt-600m-v3-int8.json').read_text())
    v3 = fixture.parent / v3_manifest['model_id']
    originals = Path.home() / '.cache/koett/model-evaluations/parakeet-v3' / v3.name
    for item in v3_manifest['files']:
        original = originals / item['path']
        assert hashlib.sha256(original.read_bytes()).hexdigest() == item['sha256']
        dest = v3 / item['path']
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(original, dest)
    ns['config'].write_text(json.dumps({'start_at_login': False, 'selected_model': 'parakeet_v3'}))
binary, output, wait, find, Atspi = [ns[x] for x in ['binary', 'output', 'wait', 'find', 'Atspi']]
process = subprocess.Popen([str(binary)], stdout=(output / 'app.log').open('w'), stderr=subprocess.STDOUT)
ns['process'] = process
try:
    main = wait(lambda: find('Koett', role='frame'), 'main')
    ns['main'] = main
    wait(lambda: ns['transcript']() == ns['texts'][0], 'selected')
    wait(lambda: find('Ready', main), 'ready', timeout=120)
    if not os.environ.get('COPY_SKIP_REPRO'):
        subprocess.run(['wl-copy'], input=b'private sentinel', check=True)
        node = find('Copy', main)
        print('before focused', node.get_state_set().contains(Atspi.StateType.FOCUSED), flush=True)
        print('action', node.get_action_iface().do_action(0), flush=True)
        time.sleep(1)
        print('label', node.get_name(), 'clipboard', subprocess.check_output(['wl-paste', '--no-newline'])[:80], flush=True)
        ns['resize']('Koett', 960, 680)
        ns['keys']('chord', 29, 33)
        time.sleep(2)
        print('action focused', find('Copy', main).get_action_iface().do_action(0), flush=True)
        wait(lambda: subprocess.check_output(['wl-paste', '--no-newline']).decode() == ns['texts'][0], 'exact native copy')
    from linux_native_copy import native_copy, stop_clipboard_owners
    ns['capture']('before-copy')
    ns['kwin']('print(JSON.stringify(workspace.windowList().map(w => ({pid:w.pid,caption:w.caption,geometry:w.frameGeometry,minimized:w.minimized}))))')
    for method in ['pointer', 'action', 'keyboard']:
        native_copy(ns, ns['texts'][0], method=method)
    def select_text(expected):
        listing = wait(lambda: next((node for node in ns['walk'](main)
            if node.get_role_name() == 'list'), None), 'History list')
        for index in range(listing.get_child_count()):
            row = listing.get_child_at_index(index)
            if any(len(node.get_name()) >= 20 and expected.replace('\n', ' ').startswith(node.get_name())
                   for node in ns['walk'](row)):
                assert listing.get_selection_iface().select_child(index)
                wait(lambda: ns['transcript']() == expected, 'matching asynchronous detail')
                return
        raise AssertionError('expected History row missing')
    ns['search']('Tack så mycket')
    wait(lambda: find(ns['texts'][4], main), 'matching search row')
    select_text(ns['texts'][4])
    wait(lambda: ns['transcript']() == ns['texts'][4], 'search changes selected record')
    native_copy(ns, ns['texts'][4], method='keyboard')
    ns['control']('settings')
    settings = wait(lambda: find('Settings', role='frame'), 'Settings open')
    def settings_active():
        settings.clear_cache()
        return settings.get_state_set().contains(Atspi.StateType.ACTIVE)
    wait(settings_active, 'modal Settings active')
    wait(lambda: len(ns['current_streams']()) == 1, 'Settings owns one preview stream')
    ns['click']('Save', settings)
    wait(lambda: not ns['current_streams'](), 'Save releases microphone preview')
    native_copy(ns, ns['texts'][4], method='pointer')
    ns['search']('no-matching-copy-fixture')
    wait(lambda: find('No matches', main), 'no matches')
    ns['click']('Clear search', main)
    wait(lambda: find(ns['texts'][1], main), 'clear restores list')
    select_text(ns['texts'][0])
    wait(lambda: ns['transcript']() == ns['texts'][0], 'selected original after clear')
    stop_clipboard_owners()
    ns['control']('quit')
    process.wait(timeout=15)
    assert process.returncode == 0
    process = subprocess.Popen([str(binary)], stdout=(output / 'restart.log').open('w'), stderr=subprocess.STDOUT)
    ns['process'] = process
    main = wait(lambda: find('Koett', role='frame'), 'restart main')
    ns['main'] = main
    wait(lambda: ns['transcript']() == ns['texts'][0], 'restart selected')
    native_copy(ns, ns['texts'][0])
    if os.environ.get('COPY_V3'):
        wait(lambda: json.loads(subprocess.check_output([str(binary), 'menu-status']))['active_engine'] == v3.name, 'v3 identity', timeout=120)
        ns['control']('start')
        wait(ns['current_streams'], 'real private CPAL stream')
        wav = originals / 'test_wavs/en.wav'
        subprocess.run(['paplay', '--device=agent_lab_audio', str(wav)], check=True)
        ns['control']('stop')
        import sqlite3
        def latest():
            with sqlite3.connect(ns['database']) as db:
                return db.execute('SELECT model,text FROM transcripts ORDER BY created_at_ms DESC LIMIT 1').fetchone()
        row = wait(lambda: latest() if latest()[0] == v3.name else None, 'new v3 row', timeout=120)
        assert 'country' in row[1].lower(), row
        with sqlite3.connect(ns['database']) as db:
            assert db.execute('SELECT COUNT(*) FROM transcripts WHERE model=?', (v3.name,)).fetchone()[0] == 1
            assert db.execute('SELECT COUNT(*) FROM transcripts').fetchone()[0] == 6
        ns['control']('show')
        select_text(row[1])
        wait(lambda: ns['transcript']() == row[1], 'selected new v3 exact text')
        native_copy(ns, row[1], name='Copy transcript', method='action')
        wait(lambda: find('Copy last transcript', main), 'recovery notice returns Last action', timeout=30)
        native_copy(ns, row[1], name='Copy last transcript', method='keyboard')
        native_copy(ns, row[1], method='action')
        assert not ns['current_streams']()
        assert not list((output / 'data').rglob('*.wav'))
        (output / 'v3-transcript.json').write_text(json.dumps({'model': row[0], 'text': row[1], 'native_copy_exact': True}))
    maps = Path(f'/proc/{process.pid}/maps').read_text()
    (output / 'loaded-maps.txt').write_text(maps)
    gtk = Path('/home/olle/.cache/koett/runtime-builds/gtk-4.24.1-21d3eb7/runtime/lib/libgtk-4.so.1')
    assert str(gtk.resolve()) in maps
    assert hashlib.sha256(gtk.read_bytes()).hexdigest() == 'a6a8876857058a9b78ef681c9f57465a2cc664a8baa9a5123c7660598c51ef5a'
    ns['capture']('native-copy')
    (output / 'results.json').write_text(json.dumps({'native_history_action_keyboard_pointer': True, 'long_unicode_exact': True, 'selection_search_clear': True, 'settings_focus_return': True, 'restart': True, 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'gtk_sha256': hashlib.sha256(gtk.read_bytes()).hexdigest()}, indent=2))
    print('PASS', flush=True)
finally:
    from linux_native_copy import stop_clipboard_owners
    stop_clipboard_owners()
    subprocess.run([str(binary), 'quit'], check=False, capture_output=True)
    process.wait(timeout=15)
    print('exit', process.returncode, flush=True)
