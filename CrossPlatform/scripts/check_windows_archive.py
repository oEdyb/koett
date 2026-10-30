#!/usr/bin/env python3
"""Verify original notices in the actual Windows release archive."""
import argparse
import json
import tempfile
import zipfile
from pathlib import Path

from test_package_linux_archive import verify_notices


def verify(archive):
    with tempfile.TemporaryDirectory(prefix='koett-windows-archive-') as directory:
        root = Path(directory)
        with zipfile.ZipFile(archive) as package:
            package.extractall(root)
        source, = [path for path in root.iterdir() if path.is_dir()]
        for name in ['Koett.exe', 'THIRD_PARTY_NOTICES.md', 'LICENSE', 'native-build-receipt.json']:
            if not (source / name).is_file():
                raise ValueError(f'Windows archive is missing {name}')
        verify_notices(source)
        return {'archive': str(archive), 'canonical_notices': True,
                'native_receipt': True, 'runtime_execution': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=Path, required=True)
    print(json.dumps(verify(parser.parse_args().archive), indent=2))
