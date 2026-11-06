#!/usr/bin/env python3
"""Verify original notices in the actual Windows release archive."""
import argparse
import json
import os
import tempfile
import zipfile
from pathlib import Path

from test_package_linux_archive import verify_notices
from check_windows_dependencies import verify as verify_dependencies
from package_windows import safe_name, verify_payload


def verify(archive):
    with tempfile.TemporaryDirectory(prefix='koett-windows-archive-', dir=os.environ.get('TMPDIR')) as directory:
        root = Path(directory)
        with zipfile.ZipFile(archive) as package:
            for entry in package.infolist():
                if not safe_name(entry.filename.rstrip('/')) or (entry.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError('Unsafe archive path or symbolic link')
            package.extractall(root)
        source, = [path for path in root.iterdir() if path.is_dir()]
        for name in ['Koett.exe', 'koett-engine.exe', 'koett-transcribe-engine.exe', 'THIRD_PARTY_NOTICES.md', 'LICENSE', 'native-build-receipt.json']:
            if not (source / name).is_file():
                raise ValueError(f'Windows archive is missing {name}')
        release = verify_payload(source)
        verify_notices(source)
        for executable in ["Koett.exe", "koett-engine.exe", "koett-transcribe-engine.exe"]:
            verify_dependencies(source / executable)
        return {'archive': str(archive), 'canonical_notices': True,
                'native_receipt': True, 'payload_hashes': True, 'architecture': release['architecture'], 'signed': False, 'runtime_execution': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=Path, required=True)
    print(json.dumps(verify(parser.parse_args().archive), indent=2))
