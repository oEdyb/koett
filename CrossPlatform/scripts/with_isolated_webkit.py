#!/usr/bin/env python3
"""Evaluate a verified extracted WebKitGTK package inside a private lab namespace."""
import argparse
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('package', type=Path)
parser.add_argument('command', nargs=argparse.REMAINDER)
args = parser.parse_args()
assert os.environ.get('AGENT_LAB_GENERATION')
assert os.environ.get('XDG_RUNTIME_DIR', '').endswith('/agent-lab/koett-ask-finish')
assert args.command
package = args.package.resolve()
assert (package / 'usr/lib/libwebkitgtk-6.0.so.4').is_file()
with tempfile.TemporaryDirectory(prefix='koett-webkit-') as temporary:
    root = Path(temporary)
    merged = root / 'lib'
    host = root / 'host'
    merged.mkdir()
    host.mkdir()
    for entry in Path('/usr/lib').iterdir():
        (merged / entry.name).symlink_to(host / entry.name)
    if (merged / 'webkitgtk-6.0').is_symlink():
        (merged / 'webkitgtk-6.0').unlink()
    shutil.copytree(package / 'usr/lib/webkitgtk-6.0', merged / 'webkitgtk-6.0')
    library = package / 'usr/lib/libwebkitgtk-6.0.so.4'
    listing = subprocess.check_output(['ldd', str(library)], env={**os.environ, 'LD_LIBRARY_PATH': str(package / 'usr/lib')}, text=True)
    assert 'not found' not in listing, listing
    libraries = [library, package / 'usr/lib/libjavascriptcoregtk-6.0.so.1', Path('/usr/lib/ld-linux-x86-64.so.2')]
    libraries += [Path(path) for path in re.findall(r'=> (/\S+)', listing)]
    for library in libraries:
        target = merged / library.name
        if target.is_symlink():
            target.unlink()
        shutil.copy(library, target)
    command = args.command[1:] if args.command[0] == '--' else args.command
    raise SystemExit(subprocess.call(['bwrap', '--bind', '/', '/', '--dev-bind', '/dev', '/dev', '--proc', '/proc', '--ro-bind', '/usr/lib', str(host), '--ro-bind', str(merged), '/usr/lib', 'env', 'LD_LIBRARY_PATH=/usr/lib:' + os.environ.get('LD_LIBRARY_PATH', ''), *command]))
