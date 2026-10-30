#!/usr/bin/env python3
"""Verify an extracted archive and isolated installation using its actual binary."""
import argparse
import hashlib
import json
import subprocess
import tarfile
import tempfile
from pathlib import Path

CANONICAL_NOTICES = Path(__file__).resolve().parents[1] / 'packaging/notices'


def verify_notices(source):
    folder = source / 'notices'
    canonical = (CANONICAL_NOTICES / 'files.json').read_bytes()
    if (folder / 'files.json').read_bytes() != canonical:
        raise ValueError('Archived notice index differs from canonical notices')
    expected = {item['path']: item['sha256'] for item in json.loads(canonical)['files']}
    actual = {str(path.relative_to(folder)): path for path in folder.rglob('*')
              if path.is_file() and path != folder / 'files.json'}
    if actual.keys() != expected.keys():
        raise ValueError('Archived notice file set differs from canonical notices')
    for name, path in actual.items():
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected[name]:
            raise ValueError(f'Archived notice content mismatch: {name}')


def run(command):
    return subprocess.run([str(item) for item in command], check=True, capture_output=True, text=True)


def verify(archive):
    with tempfile.TemporaryDirectory(prefix='koett archive $ ') as directory:
        root = Path(directory)
        with tarfile.open(archive) as package:
            package.extractall(root, filter='data')
        source, = [item for item in root.iterdir() if item.is_dir()]
        for name in ['install.py', 'install.sh', 'preflight.py', 'koett', 'libexec/koett', 'files.json',
                     'native-build-receipt.json', 'THIRD_PARTY_NOTICES.md', 'notices/files.json',
                     'notices/components.json', 'notices/models.json']:
            assert (source / name).is_file(), name
        verify_notices(source)
        report = json.loads(run([source / 'koett', '--check-runtime']).stdout)
        assert all(item['available'] for item in report['required'].values()), report
        prefix = root / 'installed space $dollar'
        old = prefix / 'bin/koett'
        old.parent.mkdir(parents=True)
        old.write_text('#!/bin/sh\nexec sleep 60\n')
        old.chmod(0o755)
        process = subprocess.Popen([str(old)], start_new_session=True)
        try:
            command = [source / 'install.sh', '--prefix', prefix]
            installed = json.loads(run(command).stdout)
            first = old.resolve()
            assert json.loads(run([old, 'menu-help']).stdout)['ok'] is True
            assert json.loads(run([old, '--check-runtime']).stdout)['required']
            run(command)
            assert old.resolve() == first
            assert list(old.parent.glob('koett.previous-*'))
            assert process.poll() is None, 'Installer stopped old process'
            with (source / 'libexec/koett').open('ab') as stream:
                stream.write(b'tamper')
            rejected = subprocess.run([str(item) for item in command], check=False, capture_output=True, text=True)
            assert rejected.returncode != 0 and 'integrity' in rejected.stderr.lower()
            assert old.resolve() == first
            # Exercise required-loader rejection before any prefix is created.
            (source / 'libexec/koett').write_text('invalid ELF fixture')
            files = json.loads((source / 'files.json').read_text())
            files['libexec/koett'] = hashlib.sha256((source / 'libexec/koett').read_bytes()).hexdigest()
            (source / 'files.json').write_text(json.dumps(files))
            missing = root / 'must not install'
            rejected = subprocess.run([str(source / 'install.sh'), '--prefix', str(missing)], check=False, capture_output=True)
            if json.loads((source / 'candidate.json').read_text())['flavor'] == 'system-runtime':
                assert rejected.returncode != 0
                assert not missing.exists()
                loader_rejection = True
            return {'missing_loader_install_rejection': locals().get('loader_rejection', 'system flavor only'), 'archive': str(archive), 'binary_sha256': json.loads((source / 'candidate.json').read_text())['binary_sha256'], 'checks': ['extraction', 'required loader', 'isolated install', 'spaces and dollar', 'launcher JSON menu-help', 'reinstall', 'backup', 'old process preserved', 'integrity rejection'], 'scope': installed['scope']}
        finally:
            if process.poll() is None:
                process.terminate()
            process.wait()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=Path, required=True)
    print(json.dumps(verify(parser.parse_args().archive), indent=2))
