#!/usr/bin/env python3
"""Link Windows artifacts locally on Linux with an already approved xwin SDK."""
import argparse
import os
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

TARGET = 'x86_64-pc-windows-msvc'
CROSS = Path(__file__).resolve().parents[1]
CACHE = Path.home() / '.cache/koett-windows'


def library_aliases(sdk, native, libraries):
    receipt = native.parent / 'build-receipt.json'
    if not receipt.exists():
        raise ValueError(f'Missing native build receipt: {receipt}')
    digest = hashlib.sha256(receipt.read_bytes()).hexdigest()
    marker = sdk / '.koett-coff-aliases.json'
    if marker.exists():
        saved = json.loads(marker.read_text())
        if saved.get('native_receipt') == digest and all(Path(path).exists() for path in saved.get('aliases', [])):
            return
    archives = sorted(native.glob('*.lib'))
    if not archives:
        raise ValueError(f'No native Windows archives in {native}')
    directives = subprocess.check_output(['llvm-readobj', '--coff-directives', *map(str, archives)], text=True)
    names = set(a or b for a, b in re.findall(r'/DEFAULTLIB:(?:"([^"]+)"|([^\s]+))', directives, re.I))
    index = {path.name.lower(): path for library in libraries for path in library.glob('*.lib')}
    aliases = []
    for name in sorted(names):
        if Path(name).name != name:
            continue
        if not name.lower().endswith('.lib'):
            name += '.lib'
        if actual := index.get(name.lower()):
            alias = actual.parent / name
            if not alias.exists():
                alias.symlink_to(actual.name)
            aliases.append(str(alias))
    marker.write_text(json.dumps({'native_receipt': digest, 'aliases': aliases}, indent=2) + '\n')


def environment(sdk, native, cache):
    sdk, native, cache = (Path(path).resolve() for path in (sdk, native, cache))
    if any(Path('/tmp') == path or Path('/tmp') in path.parents for path in (sdk, native, cache)):
        raise ValueError('Windows build inputs and output must not use /tmp')
    libraries = [sdk / 'crt/lib/x86_64', sdk / 'sdk/lib/um/x86_64', sdk / 'sdk/lib/ucrt/x86_64']
    includes = [sdk / 'crt/include', *(sdk / f'sdk/include/{name}' for name in ('ucrt', 'um', 'shared'))]
    for path in [*libraries, *includes, native / 'koett-asr-only.txt', native / 'koett-context.txt']:
        if not path.exists():
            raise ValueError(f'Missing local Windows input: {path}; see docs/windows-draw-pin.md')
    library_aliases(sdk, native, libraries)
    result = dict(os.environ)
    for name in ('DOCS_RS', 'LIBSQLITE3_SYS_USE_PKG_CONFIG', 'PKG_CONFIG_ALLOW_CROSS'):
        result.pop(name, None)
    temporary = cache / 'test-temp'
    temporary.mkdir(parents=True, exist_ok=True)
    result.update(CARGO_BUILD_JOBS='4', TMPDIR=str(temporary), SHERPA_ONNX_LIB_DIR=str(native),
                  CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=shutil.which('lld-link') or 'lld-link',
                  CC_x86_64_pc_windows_msvc=shutil.which('clang-cl') or 'clang-cl',
                  AR_x86_64_pc_windows_msvc=shutil.which('llvm-lib') or 'llvm-lib',
                  CFLAGS_x86_64_pc_windows_msvc=' '.join(f'/imsvc"{path}"' for path in includes),
                  CC_SHELL_ESCAPED_FLAGS='1',
                  CARGO_PROFILE_DEV_DEBUG='0')
    flags = ['-C', 'target-feature=+crt-static']
    for library in libraries:
        flags.extend(['-L', f'native={library}'])
    result['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(flags)
    result.pop('RUSTFLAGS', None)
    result.pop('CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS', None)
    target = Path(result.get('CARGO_TARGET_DIR', CROSS / 'target')).resolve()
    if target == Path('/tmp') or Path('/tmp') in target.parents:
        raise ValueError('CARGO_TARGET_DIR must not use /tmp')
    result['CARGO_TARGET_DIR'] = str(target)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk', type=Path, default=CACHE / 'msvc-sdk')
    parser.add_argument('--native', type=Path, default=CACHE / 'native-msvc/lib')
    parser.add_argument('cargo', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if sys.platform != 'linux':
        parser.error('this helper is for Linux cross builds; use the native Windows commands there')
    command = args.cargo or ['build', '--locked', '--bin', 'koett']
    if command[0:1] == ['--']:
        command = command[1:]
    if not command or command[0] not in ('build', 'test', 'check', 'clippy'):
        parser.error('expected a cargo build, test, check or clippy command')
    if '--target' in command:
        position = command.index('--target')
        if command[position + 1:position + 2] != [TARGET]:
            parser.error(f'this SDK/native build targets {TARGET}')
    elif not any(value.startswith('--target=') for value in command):
        command = [command[0], '--target', TARGET, *command[1:]]
    if any(value.startswith('--target=') and value != f'--target={TARGET}' for value in command):
        parser.error(f'this SDK/native build targets {TARGET}')
    try:
        env = environment(args.sdk, args.native, CACHE)
    except ValueError as error:
        parser.error(str(error))
    raise SystemExit(subprocess.call(['nice', '-n', '10', 'cargo', *command], cwd=CROSS, env=env))


if __name__ == '__main__':
    main()
