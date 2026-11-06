#!/usr/bin/env python3
"""Wrap a verified Windows payload in a per-user native installer."""
import argparse
import json
import os
import re
from pathlib import Path
import subprocess
from package_windows import digest, verify_payload


def build(payload, output, compiler):
    payload, output = Path(payload).resolve(), Path(output).resolve()
    prefix = '/' if os.name == 'nt' else '-'
    compiler_version = subprocess.check_output([compiler, prefix+'VERSION'], text=True).strip()
    if compiler_version not in ('v3.12', 'v3.12-1'):
        raise ValueError('Use the pinned NSIS 3.12 compiler')
    if output.is_relative_to(payload):
        raise ValueError('Installer output must be outside the verified payload')
    if os.name != 'nt' and (output == Path('/tmp') or Path('/tmp') in output.parents):
        raise ValueError('Build output must not use /tmp')
    release = verify_payload(payload)
    if release['architecture'] != 'x86_64':
        raise ValueError('Native ARM64 installer support is not verified')
    version = release['version']
    if not isinstance(version, str) or not re.fullmatch(r'\d+\.\d+\.\d+', version) or any(int(p) > 65535 for p in version.split('.')):
        raise ValueError('Invalid installer version')
    output.parent.mkdir(parents=True, exist_ok=True)
    script = Path(__file__).resolve().parents[1]/'packaging/windows/installer.nsi'
    subprocess.run([compiler, prefix+'WX', prefix+'V2', prefix+'DOUTPUT='+str(output),
                    prefix+'DPAYLOAD='+str(payload), prefix+'DVERSION='+version,
                    prefix+'DVERSIONKEY='+digest(payload/'preview-release.json')[:16], str(script)], check=True)
    return {'installer': str(output), 'sha256': digest(output), 'version': version,
            'architecture': release['architecture'], 'signed': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--payload', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--compiler', default='makensis')
    args = parser.parse_args()
    print(json.dumps(build(args.payload, args.output, args.compiler), indent=2))
