"""Recompute live audit claims without reading a transcript database."""
import gzip
import hashlib
import json
from pathlib import Path
import statistics

base = Path(__file__).resolve().parent

def load(folder, name):
    return json.loads((folder / name).read_text())

def lines(folder, name):
    return [json.loads(x) for x in gzip.decompress((folder / (name + '.gz')).read_bytes()).decode().splitlines()]

def no_live_text(value):
    if isinstance(value, dict):
        assert not {'text', 'transcript', 'clipboard_text', 'HOME', 'NIRI_SOCKET'} & value.keys()
        for child in value.values(): no_live_text(child)
    elif isinstance(value, list):
        for child in value: no_live_text(child)

for name in ['linux-live', 'linux-live-v1']:
    folder = base / name
    manifest = load(folder, 'files.json')
    for filename, expected in manifest.items():
        assert hashlib.sha256((folder / filename).read_bytes()).hexdigest() == expected, filename
        if filename.endswith('.json'):
            no_live_text(load(folder, filename))
        elif filename.endswith('.jsonl.gz'):
            for row in lines(folder, filename.removesuffix('.gz')): no_live_text(row)

folder = base / 'linux-live'
suite = load(folder, 'suite-final.json')
soak = load(folder, 'soak.json')
assert len(soak) == 100
assert next(x['result'] for x in suite if x['command']['name'] == 'soak') == soak
assert sum(x['command']['name'] == 'cancel' for x in suite) == 10
for seconds in [5, 30, 120, 600]:
    rows = [x['result'] for x in suite if x['command'].get('seconds') == seconds and x['command'].get('feature') == 'off' and not x['command'].get('warmup')]
    assert len(rows) == 3
    print(seconds, 'stop-to-target median/worst ms', statistics.median(x['stop_to_text_ms'] for x in rows), max(x['stop_to_text_ms'] for x in rows))

values = []
for row in lines(folder, 'cases.jsonl'):
    if not row['result']['ok']: continue
    value = row['result']['value']
    if isinstance(value, list): values.extend(value)
    elif 'stop_to_text_ms' in value: values.append(value)
events = [x for x in lines(folder, 'receiver-events.jsonl') if x['kind'] == 'text']
by_nonce = {x['nonce']: x for x in events}
assert len(by_nonce) == len(events) == 151
for value in values:
    assert value['text_event_count'] == 1
    assert int(by_nonce[value['nonce']]['time_ns']) == value['receipt_ns']
    assert abs((value['receipt_ns'] - value['stop_ns']) / 1e6 - value['stop_to_text_ms']) < 1e-6
    assert value['text_sha256'] == by_nonce[value['nonce']]['sha256']
assert len(values) + len(load(folder, 'hotkey-e2e.json')) == 151
assert load(folder, 'storage-check.json')['actual_saved'] == 151
assert load(folder, 'storage-check.json')['after_three_launches_count'] == 151
assert load(folder, 'storage-check.json')['all_wav_files_in_private_data'] == 0
continuity = {int(x['receipt_ns']): x for x in lines(folder, 'continuity.jsonl')}
for row in suite:
    if row['command'].get('seconds') == 600:
        anchors = continuity[row['result']['receipt_ns']]
        assert anchors['hash_matches']
        assert [anchors[k] for k in ['start_anchor_count', 'end_anchor_count', 'tail_anchor_count']] == [81, 81, 80]
print('100 live dictations: RSS first/last10 KiB', statistics.median(x['rss_kib'] for x in soak[:10]), statistics.median(x['rss_kib'] for x in soak[-10:]))
assert all(x['text_event_count'] == 1 for x in soak)
assert len({x['nonce'] for x in soak}) == 100
cpu = load(folder, 'cpu-final.json')
assert len(cpu['observed_worker_priorities']) == 8 and set(cpu['observed_worker_priorities']) == {10}
for mode in ['baseline', 'loaded']:
    assert len(cpu[mode]) == 3
    print('CPU', mode, statistics.median(x['stop_to_text_ms'] for x in cpu[mode]), max(x['stop_to_text_ms'] for x in cpu[mode]))
assert len(load(folder, 'launches.json')) == 3
post = load(folder, 'post-link.json')
assert post['selected_mode_shared'] and post['total_saved_count'] == 158
assert len(post['takes']) == 4 and len(post['visual']['images']) == 2
for row in post['takes']: assert row['result']['text_event_count'] == 1
print('151 main-matrix results plus7 rebuilt-binary cases saved once; no live text in exports')
