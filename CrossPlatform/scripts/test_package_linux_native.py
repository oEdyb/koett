import hashlib
import json
import os
from unittest.mock import patch
from pathlib import Path
import tarfile
import tempfile
import unittest

import package_linux_native as native


class NativePackageTests(unittest.TestCase):
    def payload(self, base):
        root = base / 'payload'
        root.mkdir()
        (root / 'libexec').mkdir()
        (root / 'libexec/koett').write_bytes(b'ELF fixture')
        (root / 'libexec/koett').chmod(0o755)
        (root / 'koett').write_text('#!/bin/sh\nexec true\n')
        (root / 'koett').chmod(0o755)
        (root / 'candidate.json').write_text(json.dumps({'flavor': 'system-runtime', 'source_revision': 'a' * 40, 'source_dirty': False}))
        (root / 'com.olledyberg.Koett.desktop').write_text('[Desktop Entry]\nExec=koett\n')
        (root / 'com.olledyberg.Koett.svg').write_text('<svg/>')
        (root / 'files.json').write_text(json.dumps({p.relative_to(root).as_posix(): native.digest(p) for p in root.rglob('*') if p.is_file()}))
        return root

    def test_system_files_have_one_owner_and_absolute_desktop_command(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            payload = self.payload(base)
            root = base / 'staged'
            native.stage(payload, root)
            self.assertEqual((root / 'usr/bin/koett').readlink(), Path('../lib/koett/koett'))
            self.assertIn('Exec=/usr/bin/koett\n', (root / 'usr/share/applications/com.olledyberg.Koett.desktop').read_text())
            self.assertFalse((root / 'usr/lib/koett/versions').exists())
            self.assertTrue((root / 'usr/share/icons/hicolor/scalable/apps/com.olledyberg.Koett.svg').exists())
            self.assertEqual((root / 'usr/lib/koett/libexec/koett').read_bytes(), b'ELF fixture')

    def test_tampered_payload_is_rejected_before_packaging(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            payload = self.payload(base)
            (payload / 'libexec/koett').write_bytes(b'changed')
            archive = base / 'release.tar.gz'
            with tarfile.open(archive, 'w:gz') as stream:
                stream.add(payload, arcname='release')
            with self.assertRaisesRegex(ValueError, 'integrity failure'):
                native.unpack(archive, base / 'extracted', native.digest(archive), 'a' * 40)

    def test_public_paths_remain_readable_under_private_umask(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            payload = self.payload(base)
            previous = os.umask(0o077)
            try:
                native.stage(payload, base / 'staged')
            finally:
                os.umask(previous)
            for path in (base / 'staged').rglob('*'):
                if not path.is_symlink():
                    self.assertEqual(path.stat().st_mode & 0o777, 0o755 if path.is_dir() or path.name == 'koett' else 0o644)

    def test_checksum_and_dirty_source_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            payload = self.payload(base)
            candidate = payload / 'candidate.json'
            value = json.loads(candidate.read_text())
            value['source_dirty'] = True
            candidate.write_text(json.dumps(value))
            archive = base / 'release.tar.gz'
            with tarfile.open(archive, 'w:gz') as stream:
                stream.add(payload, arcname='release')
            with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
                native.unpack(archive, base / 'extracted', '0' * 64, 'a' * 40)
            with self.assertRaisesRegex(ValueError, 'dirty source'):
                native.unpack(archive, base / 'extracted', native.digest(archive), 'a' * 40)

    def test_metadata_carries_clipboard_capture_and_overlay_helpers(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / 'root'
            native.stage(self.payload(base), root)
            with patch.object(native.subprocess, 'run'):
                native.build_deb(root, base / 'out.deb', '0.3.0', '1', 'x86_64')
            control = (root / 'DEBIAN/control').read_text()
            self.assertIn('xclip, libgtk4-layer-shell0', control)
            def rpmbuild(*args, **kwargs):
                out = base / 'rpm/RPMS/x86_64'
                out.mkdir(parents=True)
                (out / 'koett.rpm').write_bytes(b'package fixture')
            with patch.object(native.subprocess, 'run', side_effect=rpmbuild):
                native.build_rpm(root, base / 'out.rpm', '0.3.0', '1', 'x86_64', base)
            spec = (base / 'rpm/SPECS/koett.spec').read_text()
            self.assertIn('Recommends: wl-clipboard, wtype, grim, xclip, gtk4-layer-shell', spec)
            self.assertIn('%defattr(-,root,root,-)', spec)
            with patch.object(native.subprocess, 'run'):
                native.build_arch(root, base / 'out.pkg.tar.zst', '0.3.0', '1', 'x86_64')
            self.assertIn('depend = xclip', (root / '.PKGINFO').read_text())

    def test_valid_package_keeps_release_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            payload = self.payload(base)
            archive = base / 'release.tar.gz'
            with tarfile.open(archive, 'w:gz') as stream:
                stream.add(payload, arcname='release')
            root, candidate = native.unpack(archive, base / 'extracted', native.digest(archive), 'a' * 40)
            self.assertEqual(candidate['source_revision'], 'a' * 40)
            self.assertEqual(native.digest(root / 'libexec/koett'), hashlib.sha256(b'ELF fixture').hexdigest())


if __name__ == '__main__':
    unittest.main()
