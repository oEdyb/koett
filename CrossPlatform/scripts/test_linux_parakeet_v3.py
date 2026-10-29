#!/usr/bin/env python3
"""Verify native model selection, real download progress and cancellation privately."""
import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
from pathlib import Path

source = Path(__file__).with_name('test_linux_models.py')
bootstrap = source.read_text().split('process = None\nchecks = []\ntry:')[0]
namespace = {'__file__': str(source), '__name__': '__main__'}
exec(compile(bootstrap, str(source), 'exec'), namespace)  # noqa: S102 - shares the tracked native fixture setup
fixture_model, model, cache, archive_name, output, binary, find, wait, walk, Atspi = (
    namespace[name] for name in ['fixture_model', 'model', 'cache', 'archive_name', 'output', 'binary', 'find', 'wait', 'walk', 'Atspi']
)


def capture(name):
    namespace['process'] = process
    namespace['capture'](name)
assert fixture_model.is_symlink()
fixture_model.unlink()
manifest = json.loads((source.parents[1] / 'model-manifests/parakeet-tdt-ctc-110m-int8.json').read_text())
for item in manifest['files']:
    original = model / item['path']
    assert hashlib.sha256(original.read_bytes()).hexdigest() == item['sha256']
    destination = fixture_model / item['path']
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(original, destination)
assert (cache / archive_name).is_symlink()
(cache / archive_name).unlink()
with sqlite3.connect(namespace['database']) as connection:
    connection.execute('DELETE FROM transcripts')
process = None
try:
    log = (output / 'app.log').open('w')
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    main = wait(lambda: find('Koett', role='frame'), 'main')
    wait(lambda: find('Ready', main), 'warm compact model', timeout=120)
    subprocess.run(['gdbus', 'call', '--session', '--dest', 'com.olledyberg.Koett',
                    '--object-path', '/com/olledyberg/Koett', '--method', 'org.gtk.Actions.Activate',
                    'menu-model', "[<'parakeet_v3'>]", '{}'], check=True, capture_output=True)
    dialog = wait(lambda: find('Local models', role='frame'), 'model setup opens from selection')
    progress = wait(lambda: next((node for node in walk(dialog) if node.get_role_name() == 'progress bar' and node.get_state_set().contains(Atspi.StateType.SHOWING)), None), 'real network download progress', timeout=60)
    fraction = wait(lambda: progress.get_value_iface().get_current_value() if progress.get_value_iface().get_current_value() > 0 else None, 'nonzero download percentage', timeout=30)
    assert 0 < fraction <= 1, fraction
    bar_bounds = progress.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
    window_bounds = dialog.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
    assert window_bounds.y <= bar_bounds.y and bar_bounds.y + bar_bounds.height <= window_bounds.y + window_bounds.height
    cancel = find('Cancel model setup', dialog)
    assert cancel.get_state_set().contains(Atspi.StateType.SENSITIVE)
    capture('download-progress')
    cancel.get_action_iface().do_action(0)
    wait(lambda: not cancel.get_state_set().contains(Atspi.StateType.SENSITIVE), 'model cancellation completed', timeout=30)
    status = json.loads(subprocess.check_output([str(binary), 'menu-status'], text=True))
    assert status['active_engine'] == manifest['model_id'], status['active_engine']
    assert status['desired_model'] == 'compact', status['desired_model']
    assert not status['busy'], status
    capture('download-cancelled')
    # Optional verified local files speed repeated capture checks after the network proof.
    if os.environ.get('KOETT_V3_VERIFIED_FIXTURE'):
        shutil.copytree(Path(os.environ['KOETT_V3_VERIFIED_FIXTURE']), fixture_model.parent / 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8')
    # Finish the actual native setup after proving network cancellation.
    subprocess.run([str(binary), 'menu-model', 'parakeet_v3'], check=True, capture_output=True)
    wait(lambda: json.loads(subprocess.check_output([str(binary), 'menu-status'], text=True)).get('desired_model') == 'parakeet_v3', 'v3 installed and selected', timeout=240)
    wait(lambda: find('Ready', dialog), 'v3 ready', timeout=120)
    status = json.loads(subprocess.check_output([str(binary), 'menu-status'], text=True))
    assert status['active_engine'] == 'sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8', status
    capture('v3-active')
    # A failed preference write must preserve the warm v3 engine.
    config = namespace['config']
    config.parent.chmod(0o500)
    try:
        subprocess.run([str(binary), 'menu-model', 'compact'], check=True, capture_output=True)
        wait(lambda: any('could not' in node.get_name() for node in walk(dialog)), 'failed preference write', timeout=120)
        wait(lambda: find('Ready', dialog), 'v3 retained', timeout=120)
        assert json.loads(subprocess.check_output([str(binary), 'menu-status'], text=True))['active_engine'] == status['active_engine']
    finally:
        config.parent.chmod(0o700)
    subprocess.run([str(binary), 'quit'], check=True, capture_output=True)
    process.wait(timeout=15)
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    wait(lambda: json.loads(subprocess.check_output([str(binary), 'menu-status'], text=True)).get('active_engine') == status['active_engine'], 'restart v3 identity', timeout=120)
    assert json.loads(config.read_text())['selected_model'] == 'parakeet_v3'
    database = namespace['database']
    with sqlite3.connect(database) as connection:
        connection.execute('DELETE FROM transcripts')
    namespace['control']('start')
    def streams():
        clients = json.loads(subprocess.check_output(['pactl', '-f', 'json', 'list', 'clients']))
        owned = {str(c['index']) for c in clients if str(c.get('properties', {}).get('application.process.id')) == str(process.pid)}
        return [x for x in json.loads(subprocess.check_output(['pactl', '-f', 'json', 'list', 'source-outputs'])) if str(x.get('client')) in owned]
    wait(streams, 'private CPAL stream')
    wav = Path.home() / '.cache/koett/model-evaluations/parakeet-v3/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/test_wavs/en.wav'
    subprocess.run(['paplay', '--device=agent_lab_audio', str(wav)], check=True)
    namespace['control']('stop')
    def row():
        with sqlite3.connect(database) as connection:
            rows = connection.execute('SELECT model,text FROM transcripts').fetchall()
        return rows[0] if len(rows) == 1 else None
    transcript = wait(row, 'exactly one v3 history row', timeout=120)
    assert transcript[0] == status['active_engine'], transcript
    assert 'country' in transcript[1].lower(), transcript
    main = wait(lambda: find('Koett', role='frame'), 'history window')
    namespace['control']('show')
    namespace['main'] = main
    wait(lambda: namespace['transcript']() == transcript[1], 'selected v3 detail')
    namespace['keys']('chord', 29, 33)
    subprocess.run(['wl-copy'], input='private clipboard sentinel', text=True, check=True)
    response = json.loads(subprocess.check_output([str(binary), 'copy-last'], text=True))
    assert response['ok'], response
    wait(lambda: subprocess.check_output(['wl-paste', '--no-newline'], text=True) == transcript[1], 'agent Copy last replaces sentinel')
    assert not streams()
    assert not list((output / 'data').rglob('*.wav'))
    capture('v3-history-copy')
    (output / 'transcript.json').write_text(json.dumps({'model':transcript[0], 'text':transcript[1], 'copy_exact':True, 'copy_surface':'agent copy-last', 'native_copy_button_verified':False, 'history_rows':1}))
    (output / 'results.json').write_text(json.dumps({'native_model_selection_started_setup':True, 'download_progress_visible':True, 'fraction_observed':fraction, 'cancel_enabled':True, 'cancel_retains_compact_engine':True, 'preference_unchanged_after_cancel':True, 'v3_loaded_and_restart_persisted':True, 'failed_setup_retained_v3':True, 'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}, indent=2))
finally:
    if process is not None:
        subprocess.run([str(binary), 'quit'], check=False, capture_output=True)
        process.wait(timeout=15)
