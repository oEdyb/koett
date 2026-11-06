#!/usr/bin/env python3
"""Build distro packages from a verified system-runtime release payload."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import shlex
import struct
import subprocess
import tarfile
import tempfile

ARCHES = {'x86_64': ('amd64', 'x86_64'), 'aarch64': ('arm64', 'aarch64')}


def digest(path):
    with path.open('rb') as stream:
        value = hashlib.sha256()
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
        return value.hexdigest()


def unpack(archive, destination):
    with tarfile.open(archive) as source:
        source.extractall(destination, filter='data')
    roots = list(destination.iterdir())
    if len(roots) != 1 or not roots[0].is_dir():
        raise ValueError('Expected one release payload directory')
    root = roots[0]
    candidate = json.loads((root / 'candidate.json').read_text())
    if candidate['flavor'] != 'system-runtime':
        raise ValueError('Native packages require the portable system runtime')
    manifest = json.loads((root / 'files.json').read_text())
    actual = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}
    if actual != set(manifest) | {'files.json'}:
        raise ValueError('Payload contains missing or unlisted files')
    for name, expected in manifest.items():
        path = root / name
        if not path.resolve().is_relative_to(root.resolve()) or digest(path) != expected:
            raise ValueError(f'Payload integrity failure: {name}')
    return root, candidate


def stage(payload, root):
    app = root / 'usr/lib/koett'
    shutil.copytree(payload, app)
    binary = root / 'usr/bin/koett'
    binary.parent.mkdir(parents=True)
    binary.symlink_to('../lib/koett/koett')
    desktop = root / 'usr/share/applications/com.olledyberg.Koett.desktop'
    desktop.parent.mkdir(parents=True)
    desktop.write_text((payload / desktop.name).read_text().replace('Exec=koett\n', 'Exec=/usr/bin/koett\n'))
    icon = root / 'usr/share/icons/hicolor/scalable/apps/com.olledyberg.Koett.svg'
    icon.parent.mkdir(parents=True)
    shutil.copy2(payload / icon.name, icon)


def build_deb(root, output, version, revision, arch):
    control = root / 'DEBIAN'
    control.mkdir()
    (control / 'control').write_text(f'''Package: koett
Version: {version}-{revision}
Architecture: {ARCHES[arch][0]}
Maintainer: Koett maintainers
Section: sound
Priority: optional
Depends: python3 (>= 3.10), libgtk-4-1 (>= 4.6), libasound2, libudev1, libxkbcommon0, libwayland-client0, libx11-6, libxcb-shape0, libxcb-xfixes0, libstdc++6, zlib1g
Recommends: wl-clipboard, wtype, grim
Homepage: https://github.com/oEdyb/koett
Description: Local dictation with screen drawing and pins
 Native voice-to-text application. Models are downloaded on first use.
''')
    subprocess.run(['dpkg-deb', '--root-owner-group', '--build', str(root), str(output)], check=True)


def build_rpm(root, output, version, revision, arch, work):
    top = work / 'rpm'
    for folder in ['BUILD', 'BUILDROOT', 'RPMS', 'SOURCES', 'SPECS', 'SRPMS']:
        (top / folder).mkdir(parents=True, exist_ok=True)
    spec = top / 'SPECS/koett.spec'
    spec.write_text(f'''Name: koett
Version: {version}
Release: {revision}
Summary: Local dictation with screen drawing and pins
License: Apache-2.0 AND LicenseRef-Koett-Third-Party
URL: https://github.com/oEdyb/koett
BuildArch: {arch}
Requires: python3 >= 3.10, gtk4 >= 4.6, libasound.so.2()(64bit), libudev.so.1()(64bit), libxkbcommon.so.0()(64bit), libwayland-client.so.0()(64bit), libX11.so.6()(64bit), libxcb.so.1()(64bit), libstdc++.so.6()(64bit), libz.so.1()(64bit)
AutoReqProv: no
%description
Native local voice-to-text application with screen drawing and pins.
%install
mkdir -p %{{buildroot}}
cp -a {shlex.quote(str(root / "usr"))} %{{buildroot}}/
%files
/usr/bin/koett
/usr/lib/koett
/usr/share/applications/com.olledyberg.Koett.desktop
/usr/share/icons/hicolor/scalable/apps/com.olledyberg.Koett.svg
''')
    subprocess.run(['rpmbuild', '-bb', '--define', f'_topdir {top}', '--define', '_build_id_links none', '--define', '__os_install_post %{nil}', str(spec)], check=True)
    packages = list((top / 'RPMS').rglob('*.rpm'))
    if len(packages) != 1:
        raise ValueError('Expected one RPM')
    shutil.copy2(packages[0], output)


def build_arch(root, output, version, revision, arch):
    size = sum(p.stat().st_size for p in root.rglob('*') if p.is_file())
    dependencies = ['python>=3.10', 'gtk4>=4.6', 'alsa-lib', 'systemd-libs', 'libxkbcommon', 'wayland', 'libx11', 'libxcb', 'gcc-libs', 'zlib']
    info = f'''pkgname = koett
pkgbase = koett
pkgver = {version}-{revision}
pkgdesc = Local dictation with screen drawing and pins
url = https://github.com/oEdyb/koett
builddate = 0
packager = Koett maintainers
size = {size}
arch = {arch}
license = Apache-2.0
'''
    info += ''.join(f'depend = {dep}\n' for dep in dependencies)
    info += 'optdepend = gtk4-layer-shell: drawing overlay\noptdepend = grim: drawing capture on wlroots\noptdepend = wl-clipboard: Wayland clipboard\noptdepend = wtype: Wayland paste\n'
    (root / '.PKGINFO').write_text(info)
    subprocess.run(['bsdtar', '--zstd', '--uid', '0', '--gid', '0', '-cf', str(output), '-C', str(root), '.PKGINFO', 'usr'], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--format', choices=['deb', 'rpm', 'arch'], required=True)
    parser.add_argument('--architecture', choices=ARCHES, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--revision', default='1')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r'[0-9]+(?:\.[0-9]+){2}', args.version) or not re.fullmatch(r'[1-9][0-9]*', args.revision):
        parser.error('Use a numeric three-part version and positive revision')
    args.output.mkdir(parents=True, exist_ok=True)
    output = args.output.resolve()
    with tempfile.TemporaryDirectory(prefix='native-package-', dir=output) as directory:
        work = Path(directory)
        extracted = work / 'payload'
        extracted.mkdir()
        payload, candidate = unpack(args.archive, extracted)
        elf = (payload / 'libexec/koett').read_bytes()[:20]
        machine = struct.unpack_from('<H', elf, 18)[0] if elf[:5] == b'\x7fELF\x02' else 0
        if {62: 'x86_64', 183: 'aarch64'}.get(machine) != args.architecture:
            raise ValueError('Package architecture differs from payload')
        root = work / 'root'
        stage(payload, root)
        arch = args.architecture
        suffix = {'deb': f'{ARCHES[arch][0]}.deb', 'rpm': f'{arch}.rpm', 'arch': f'{arch}.pkg.tar.zst'}[args.format]
        artifact = output / f'koett-{args.version}-{args.revision}-{suffix}'
        if artifact.exists():
            raise ValueError(f'Refusing to overwrite {artifact}')
        if args.format == 'deb':
            build_deb(root, artifact, args.version, args.revision, arch)
        elif args.format == 'rpm':
            build_rpm(root, artifact, args.version, args.revision, arch, work)
        else:
            build_arch(root, artifact, args.version, args.revision, arch)
        Path(str(artifact) + '.sha256').write_text(f'{digest(artifact)}  {artifact.name}\n')
        print(json.dumps({'artifact': str(artifact), 'source_revision': candidate['source_revision'], 'ownership': 'system package manager; private updater inactive'}))


if __name__ == '__main__':
    main()
