#!/usr/bin/env python3
"""Verify installed CLI preferences, Last and private history survive restart."""
import argparse
import json
import os
import sqlite3
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--state', type=Path, required=True)
args = parser.parse_args()
assert os.environ.get('AGENT_LAB_RUNTIME', '').endswith('/koett-package-currentpc')
os.environ.pop('NIRI_SOCKET', None)
os.environ.pop('AT_SPI_BUS_ADDRESS', None)
for kind in ['CONFIG', 'DATA', 'CACHE', 'STATE']:
    os.environ[f'XDG_{kind}_HOME'] = str(args.state / kind.lower())
database = args.state / 'data/koett/History.sqlite3'
with sqlite3.connect(database) as connection:
    count = connection.execute('SELECT count(*) FROM transcripts').fetchone()[0]
assert count > 0


def request(*command):
    result = subprocess.run([str(args.binary), *command], capture_output=True, text=True, timeout=5, check=True)
    return json.loads(result.stdout)


checks = []
with (args.state / 'restart.log').open('w') as log:
    for attempt in range(2):
        child = subprocess.Popen([str(args.binary)], stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 60
            while True:
                try:
                    status = request('menu-status')
                    if status['active_engine']:
                        break
                except (subprocess.SubprocessError, ValueError):
                    pass
                assert child.poll() is None and time.monotonic() < deadline
                time.sleep(0.2)
            recovery = request('recovery-status')
            assert recovery['ok'] and recovery['last_available']
            assert request('copy-last')['ok']
            copied = subprocess.check_output(['wl-paste', '--no-newline'])
            assert copied
            if attempt == 0:
                assert request('menu-mode', 'hold')['ok']
                first_copy = copied
            else:
                assert status['recording_mode'] == 'hold'
                assert copied == first_copy
            checks.append({'attempt': attempt, 'menu': status, 'recovery': recovery})
            request('quit')
            assert child.wait(timeout=15) == 0
        finally:
            if child.poll() is None:
                child.terminate()
                child.wait(timeout=5)
with sqlite3.connect(database) as connection:
    assert connection.execute('SELECT count(*) FROM transcripts').fetchone()[0] == count
(args.state / 'restart-results.json').write_text(json.dumps({'checks': checks, 'history_rows_unchanged': count}, indent=2))
