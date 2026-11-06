import hashlib
import json
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
        (root / 'koett').write_text('#!/bin/sh\nexec true\n')
        (root / 'koett').chmod(0o755)
        (root / 'candidate.json').write_text(json.dumps({'flavor': 'system-runtime', 'source_revision': 'a' * 40}))
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
                native.unpack(archive, base / 'extracted')

    def test_valid_package_keeps_release_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            payload = self.payload(base)
            archive = base / 'release.tar.gz'
            with tarfile.open(archive, 'w:gz') as stream:
                stream.add(payload, arcname='release')
            root, candidate = native.unpack(archive, base / 'extracted')
            self.assertEqual(candidate['source_revision'], 'a' * 40)
            self.assertEqual(native.digest(root / 'libexec/koett'), hashlib.sha256(b'ELF fixture').hexdigest())


if __name__ == '__main__':
    unittest.main()
