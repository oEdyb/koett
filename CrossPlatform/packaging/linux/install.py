#!/usr/bin/env python3
"""Private, versioned installation; never stop processes or touch configuration."""
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def sha(path):
    with path.open('rb') as stream:
        value = hashlib.sha256()
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
        return value.hexdigest()


def atomic_text(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode='w', dir=path.parent, delete=False) as stream:
        stream.write(content)
        temporary = Path(stream.name)
    temporary.chmod(0o644)
    temporary.replace(path)


def desktop_exec(path):
    argument = str(path).replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%')
    quoted = '"' + argument + '"'
    return '/usr/bin/env -- ' + quoted.replace('\\', '\\\\').replace('\n', '\\n').replace('\r', '\\r').replace('\t', '\\t')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', type=Path, default=Path.home() / '.local')
    args = parser.parse_args()
    prefix = args.prefix.absolute()
    if '=' in str(prefix):
        parser.error('Choose an installation prefix without an equals sign')
    source = Path(__file__).resolve().parent
    versions = (prefix / 'lib/koett/versions').resolve()
    if versions.is_relative_to(source):
        parser.error('Choose an installation prefix outside the extracted package')
    manifest = json.loads((source / 'files.json').read_text())
    for name, expected in manifest.items():
        path = source / name
        if not path.resolve().is_relative_to(source) or sha(path) != expected:
            raise ValueError(f'Package integrity failure: {name}')
    receipt = json.loads((source / 'candidate.json').read_text()) if (source / 'candidate.json').exists() else {}
    if receipt.get('flavor') == 'system-runtime':
        subprocess.run([sys.executable, str(source / 'preflight.py')], check=True, stdout=sys.stderr)
    identity = sha(source / 'files.json')[:24]
    versions.mkdir(parents=True, exist_ok=True)
    version = versions / identity
    if not version.exists():
        staging = Path(tempfile.mkdtemp(prefix='.install-', dir=versions))
        try:
            shutil.copytree(source, staging, dirs_exist_ok=True)
            staging.rename(version)
        finally:
            if staging.exists():
                shutil.rmtree(staging)
    else:
        for name, expected in manifest.items():
            if sha(version / name) != expected:
                raise ValueError(f'Existing version integrity failure: {name}')
    binary = prefix / 'bin/koett'
    binary.parent.mkdir(parents=True, exist_ok=True)
    if binary.exists() and not binary.is_symlink():
        backup = binary.with_name('koett.previous-' + sha(binary)[:16])
        if not backup.exists():
            shutil.copy2(binary, backup)
    target = version / 'koett'
    with tempfile.TemporaryDirectory(prefix='.koett-', dir=binary.parent) as temporary:
        link = Path(temporary) / 'koett'
        link.symlink_to(target)
        link.replace(binary)
    desktop = (source / 'com.olledyberg.Koett.desktop').read_text()
    desktop = '\n'.join('Exec=' + desktop_exec(binary) if line.startswith('Exec=') else line for line in desktop.splitlines()) + '\n'
    atomic_text(prefix / 'share/applications/com.olledyberg.Koett.desktop', desktop)
    icon = prefix / 'share/icons/hicolor/scalable/apps/com.olledyberg.Koett.svg'
    atomic_text(icon, (source / icon.name).read_text())
    print(json.dumps({'launcher': str(binary), 'version': str(version), 'scope': receipt.get('flavor', 'current-PC private candidate')}))


if __name__ == '__main__':
    main()
