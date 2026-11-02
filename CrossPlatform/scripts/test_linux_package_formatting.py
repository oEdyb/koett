#!/usr/bin/env python3
"""Bounded packaged llama-server inference using a verified private public model."""
import argparse
import hashlib
import json
import subprocess
import time
from pathlib import Path
from urllib.request import Request, urlopen

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--server', type=Path, required=True)
parser.add_argument('--model', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
with args.model.open('rb') as stream:
    assert hashlib.file_digest(stream, 'sha256').hexdigest() == '3b41ebe2502cbd03e811d5d16b022f5ab551eda58d62597d152f89535003c634'
args.output.mkdir(parents=True, exist_ok=True)
with (args.output / 'server.log').open('w') as log:
    child = subprocess.Popen([str(args.server), '-m', str(args.model), '--host', '127.0.0.1', '--port', '19388', '-t', '2', '-ngl', '0', '-c', '2048'], stdout=log, stderr=log)
    try:
        deadline = time.monotonic() + 90
        while True:
            try:
                with urlopen('http://127.0.0.1:19388/health', timeout=2) as response:
                    assert response.status == 200
                break
            except OSError:
                assert child.poll() is None
                assert time.monotonic() < deadline
                time.sleep(0.2)
        request = Request('http://127.0.0.1:19388/completion', data=json.dumps({'prompt': 'Correct punctuation: hello world', 'n_predict': 24, 'temperature': 0}).encode(), headers={'Content-Type': 'application/json'})
        with urlopen(request, timeout=60) as response:
            result = json.load(response)
        assert result['content'].strip()
        (args.output / 'result.json').write_text(json.dumps(result, indent=2))
    finally:
        child.terminate()
        child.wait(timeout=10)
