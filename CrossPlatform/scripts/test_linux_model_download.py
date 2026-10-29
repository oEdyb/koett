#!/usr/bin/env python3
"""Verify native model selection, real download progress and cancellation privately."""
import hashlib
import json
import shutil
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
process = None
try:
    log = (output / 'app.log').open('w')
    process = subprocess.Popen([str(binary)], stdout=log, stderr=log)
    main = wait(lambda: find('Koett', role='frame'), 'main')
    wait(lambda: find('Ready', main), 'warm compact model', timeout=120)
    subprocess.run(['gdbus', 'call', '--session', '--dest', 'com.olledyberg.Koett',
                    '--object-path', '/com/olledyberg/Koett', '--method', 'org.gtk.Actions.Activate',
                    'menu-model', "[<'parakeet_v2'>]", '{}'], check=True, capture_output=True)
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
    (output / 'results.json').write_text(json.dumps({'native_model_selection_started_setup':True, 'download_progress_visible':True, 'fraction_observed':fraction, 'cancel_enabled':True, 'cancel_retains_compact_engine':True, 'preference_unchanged':True, 'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}, indent=2))
finally:
    if process is not None:
        subprocess.run([str(binary), 'quit'], check=False, capture_output=True)
        process.wait(timeout=15)
