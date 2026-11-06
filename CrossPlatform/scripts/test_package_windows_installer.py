import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch
from package_windows_installer import build

CROSS = Path(__file__).resolve().parents[1]


class InstallerPolicyTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.payload = self.root/'payload'
        self.payload.mkdir()
        (self.payload/'preview-release.json').write_text('{}')
        shutil.copy2(CROSS/'packaging/windows/NSIS-COPYING.txt', self.payload/'NSIS-COPYING.txt')
        shutil.copy2(CROSS/'scripts/manage_windows_preview.ps1', self.payload/'manage-preview.ps1')
        self.release = {'version': '0.3.0', 'architecture': 'x86_64'}

    def attempt(self, output=None, compiler_version='v3.12'):
        with patch('package_windows_installer.subprocess.check_output', return_value=compiler_version), patch('package_windows_installer.verify_payload', return_value=self.release), patch('package_windows_installer.subprocess.run') as run:
            with self.assertRaises(ValueError):
                build(self.payload, output or self.root/'Setup.exe', 'makensis')
            run.assert_not_called()

    def test_other_compiler_versions_are_refused(self):
        self.attempt(compiler_version='v3.11')

    def test_unverified_architectures_and_invalid_versions_are_refused(self):
        self.release['architecture'] = 'aarch64'
        self.attempt()
        self.release['architecture'] = 'x86_64'
        for version in ['0.3.0\nFile /r secret', '0..3', '0.3.70000']:
            self.release['version'] = version
            self.attempt()

    def test_output_cannot_mutate_verified_payload(self):
        self.attempt(output=self.payload/'Setup.exe')

    def test_missing_or_modified_runtime_notice_and_manager_are_refused(self):
        for name in ['NSIS-COPYING.txt', 'manage-preview.ps1']:
            original = (self.payload/name).read_bytes()
            (self.payload/name).write_text('modified')
            self.attempt()
            (self.payload/name).unlink()
            self.attempt()
            (self.payload/name).write_bytes(original)


if __name__ == '__main__':
    unittest.main()
