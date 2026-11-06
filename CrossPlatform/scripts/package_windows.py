#!/usr/bin/env python3
"""Create a verified unsigned Windows preview archive without downloading tools."""
import argparse
import hashlib
import json
import re
from pathlib import Path, PurePosixPath
import shutil
import struct
import subprocess
import zipfile

CROSS = Path(__file__).resolve().parents[1]
EXES = ['Koett.exe', 'koett-engine.exe', 'koett-transcribe-engine.exe']


def digest(path):
    result = hashlib.sha256()
    with Path(path).open('rb') as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b''):
            result.update(chunk)
    return result.hexdigest()


def architecture(path):
    data = Path(path).read_bytes()
    if data[:2] != b'MZ':
        raise ValueError('Invalid Windows executable')
    offset, = struct.unpack_from('<I', data, 60)
    if data[offset:offset+4] != b'PE\0\0':
        raise ValueError('Invalid PE signature')
    machine, = struct.unpack_from('<H', data, offset+4)
    return {0x8664: 'x86_64', 0xaa64: 'aarch64'}[machine]


def safe_name(name):
    parts = PurePosixPath(name).parts
    return (bool(parts) and not name.startswith('/') and '\\' not in name
            and all(part not in ('.', '..')
                    and not any(ord(char) < 32 or char in '<>:"|?*' for char in part)
                    and not part.endswith(('.', ' '))
                    and not re.match(r'^(con|prn|aux|nul|com[1-9\u00b9\u00b2\u00b3]|lpt[1-9\u00b9\u00b2\u00b3])(?:\.|$)', part, re.I)
                    for part in parts)
            and PurePosixPath(name).as_posix() == name)


def verify_payload(root):
    root = Path(root)
    release = json.loads((root/'preview-release.json').read_text())
    if release.get('format') != 1 or release.get('architecture') not in ('x86_64', 'aarch64'):
        raise ValueError('Unsupported preview format or architecture')
    files = release['files']
    if not isinstance(files, dict) or not all(safe_name(name) for name in files):
        raise ValueError('Unsafe payload path')
    if any(path.is_symlink() for path in root.rglob('*')):
        raise ValueError('Payload contains a symbolic link')
    actual = {path.relative_to(root).as_posix() for path in root.rglob('*') if path.is_file()}
    if actual != set(files) | {'preview-release.json'}:
        raise ValueError('Payload contains missing or unlisted files')
    for name, expected in files.items():
        path = root/name
        if path.is_symlink() or digest(path) != expected:
            raise ValueError(f'Payload hash mismatch: {name}')
    for name in EXES:
        if architecture(root/name) != release['architecture']:
            raise ValueError(f'Executable architecture mismatch: {name}')
    if release.get('signature') != 'unsigned-preview':
        raise ValueError('Preview must not claim a verified signature')
    return release


def package(binaries, native_receipt, output):
    # Only archive creation needs TOML; validators also run on Python 3.10.
    import tomllib

    binaries, output = Path(binaries), Path(output)
    arch = architecture(binaries/'koett.exe')
    root = output/f'Koett-Windows-{arch}'
    if root.exists():
        raise ValueError(f'Refusing to replace existing package folder: {root}')
    root.mkdir(parents=True)
    for name in EXES:
        shutil.copy2(binaries/name.lower(), root/name)
    shutil.copy2(CROSS/'packaging/windows/README.md', root/'README.md')
    shutil.copy2(CROSS/'THIRD_PARTY_NOTICES.md', root/'THIRD_PARTY_NOTICES.md')
    shutil.copy2(CROSS.parent/'LICENSE', root/'LICENSE')
    shutil.copytree(CROSS/'packaging/notices', root/'notices')
    shutil.copy2(native_receipt, root/'native-build-receipt.json')
    shutil.copy2(CROSS/'scripts/manage_windows_preview.ps1', root/'manage-preview.ps1')
    (root/'sources').mkdir()
    for name in ['build_asr_native.py', 'shared_context_patch.py', 'shared_context.patch']:
        shutil.copy2(CROSS/'scripts'/name, root/'sources'/name)
    release = {
        'format': 1, 'version': tomllib.loads((CROSS/'Cargo.toml').read_text())['package']['version'],
        'architecture': arch, 'signature': 'unsigned-preview', 'media_tools_bundled': False,
        'source': subprocess.check_output(['git', '-C', str(CROSS), 'rev-parse', 'HEAD'], text=True).strip(),
        'source_dirty': bool(subprocess.check_output(['git', '-C', str(CROSS), 'status', '--porcelain'], text=True)),
        'files': {path.relative_to(root).as_posix(): digest(path) for path in sorted(root.rglob('*')) if path.is_file()},
    }
    (root/'preview-release.json').write_text(json.dumps(release, indent=2)+'\n')
    verify_payload(root)
    archive = root.with_suffix('.zip')
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as handle:
        for path in sorted(root.rglob('*')):
            if path.is_file():
                handle.write(path, path.relative_to(output))
    return {'archive': str(archive), 'sha256': digest(archive), 'architecture': arch, 'signed': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binaries', type=Path, default=CROSS/'target/release')
    parser.add_argument('--native-receipt', type=Path, default=CROSS/'native-build/build-receipt.json')
    parser.add_argument('--output', type=Path, default=CROSS/'dist')
    args = parser.parse_args()
    print(json.dumps(package(args.binaries, args.native_receipt, args.output), indent=2))
