import hashlib
import json
from pathlib import Path
import struct
import os
import subprocess
import tempfile
import tomllib
import xml.etree.ElementTree as ET
import unittest
from unittest.mock import patch
from package_windows import EXES, architecture, safe_name, verify_payload


class WindowsPayloadTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        pe = bytearray(256)
        pe[:2] = b'MZ'
        struct.pack_into('<I', pe, 60, 128)
        pe[128:132] = b'PE\0\0'
        struct.pack_into('<H', pe, 132, 0x8664)
        for name in EXES:
            (self.root/name).write_bytes(pe)
        self.release = {'format': 1, 'architecture': 'x86_64', 'signature': 'unsigned-preview',
                        'files': {name: hashlib.sha256(pe).hexdigest() for name in EXES}}
        self.save()

    def save(self):
        (self.root/'preview-release.json').write_text(json.dumps(self.release))

    def test_native_manifest_version_matches_cargo(self):
        cross = Path(__file__).resolve().parents[1]
        source_version = tomllib.loads((cross/'Cargo.toml').read_text())['package']['version']
        target = cross/'target'
        target.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=target) as temporary:
            output = Path(temporary)
            executable = output/('manifest-build.exe' if os.name == 'nt' else 'manifest-build')
            subprocess.run(['rustc', str(cross/'build.rs'), '-o', str(executable)], check=True)
            for version in [source_version, '9.8.7-preview.1']:
                env = dict(os.environ, CARGO_CFG_TARGET_OS='windows',
                           CARGO_MANIFEST_DIR=str(cross), CARGO_PKG_VERSION=version,
                           OUT_DIR=str(output))
                env.pop('CARGO_FEATURE_ASK', None)
                result = subprocess.run([str(executable)], env=env, check=True,
                                        capture_output=True, text=True)
                manifest = ET.parse(output/'Koett.manifest')
                identity = manifest.getroot().find('{urn:schemas-microsoft-com:asm.v1}assemblyIdentity')
                self.assertEqual(identity.attrib['version'], version.split('-')[0]+'.0')
                self.assertIn('/MANIFESTINPUT:'+str(output/'Koett.manifest'), result.stdout)

    def test_checksum_verified_payload_is_explicitly_unsigned(self):
        self.assertEqual(verify_payload(self.root)['signature'], 'unsigned-preview')
        self.assertEqual(architecture(self.root/EXES[0]), 'x86_64')

    def test_tampered_executable_and_unlisted_files_are_refused(self):
        (self.root/'extra.dll').write_text('unlisted')
        with self.assertRaisesRegex(ValueError, 'unlisted'):
            verify_payload(self.root)
        (self.root/'extra.dll').unlink()
        (self.root/EXES[0]).write_text('tampered')
        with self.assertRaisesRegex(ValueError, 'hash mismatch'):
            verify_payload(self.root)

    def test_paths_cannot_escape_or_use_windows_alternate_streams(self):
        for name in ['../x', '/root/x', 'C:/x', 'a\\b', 'a:b', 'a./x', 'a /x', 'a//x',
                     'CON', 'aux.txt', 'a/com1.wav', 'a?b', 'a\x00b', 'lpt².txt']:
            self.assertFalse(safe_name(name), name)
        self.release['files']['../x'] = '0'*64
        self.save()
        with self.assertRaisesRegex(ValueError, 'Unsafe'):
            verify_payload(self.root)

    def test_architecture_mismatch_is_refused(self):
        self.release['architecture'] = 'aarch64'
        self.save()
        with self.assertRaisesRegex(ValueError, 'architecture mismatch'):
            verify_payload(self.root)

    def test_symlinks_and_signature_claims_are_refused(self):
        (self.root/'linked').write_text('link fixture')
        with patch.object(Path, 'is_symlink', lambda path: path.name == 'linked'):
            with self.assertRaisesRegex(ValueError, 'symbolic link'):
                verify_payload(self.root)
        (self.root/'linked').unlink()
        self.release['signature'] = 'signed'
        self.save()
        with self.assertRaisesRegex(ValueError, 'signature'):
            verify_payload(self.root)


if __name__ == '__main__':
    unittest.main()
