"""Exercise installer integrity, quoted desktop paths and atomic replacement."""
import importlib.util
import json
import os
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('installer', ROOT / 'packaging/linux/install.py')
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class InstallTests(unittest.TestCase):
    def test_invalid_prefix_is_rejected_before_deployment(self):
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory) / 'invalid=prefix'
            result = subprocess.run(['python3', str(ROOT / 'packaging/linux/install.py'), '--prefix', str(prefix)], check=False, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('without an equals sign', result.stderr)
            self.assertFalse(prefix.exists())

    def test_desktop_launch_special_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for index, name in enumerate(['space path', 'dollar$path', 'back\\slash', 'quote"path', 'back`tick', 'percent%path']):
                with self.subTest(path=name):
                    executable = root / name / 'koett'
                    executable.parent.mkdir()
                    executable.write_text('#!/bin/sh\nprintf installed > "$KOETT_INSTALL_TEST_MARKER"\n')
                    executable.chmod(0o755)
                    desktop = root / f'fixture-{index}.desktop'
                    desktop.write_text('[Desktop Entry]\nType=Application\nName=Installer fixture\nExec=' + installer.desktop_exec(executable) + '\nTerminal=false\n')
                    marker = root / f'marker-{index}'
                    subprocess.run(['dbus-run-session', '--', 'gio', 'launch', str(desktop)], env={**os.environ, 'KOETT_INSTALL_TEST_MARKER': str(marker)}, check=True, capture_output=True)
                    deadline = time.monotonic() + 3
                    while not marker.exists() and time.monotonic() < deadline:
                        time.sleep(0.02)
                    self.assertEqual(marker.read_text(), 'installed')

    def test_integrity_spaces_and_reinstall(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / 'package with spaces'
            package.mkdir()
            for name in ['install.py', 'com.olledyberg.Koett.desktop', 'com.olledyberg.Koett.svg']:
                (package / name).write_bytes((ROOT / 'packaging/linux' / name).read_bytes())
            (package / 'koett').write_text('fixture; never executed')
            files = {p.name: installer.sha(p) for p in package.iterdir()}
            (package / 'files.json').write_text(json.dumps(files))
            prefix = root / 'private prefix'
            command = ['python3', str(package / 'install.py'), '--prefix', str(prefix)]
            subprocess.run(command, check=True, capture_output=True)
            target = (prefix / 'bin/koett').resolve()
            subprocess.run(command, check=True, capture_output=True)
            self.assertEqual(target, (prefix / 'bin/koett').resolve())
            self.assertIn(f'Exec=/usr/bin/env -- "{prefix}/bin/koett"', (prefix / 'share/applications/com.olledyberg.Koett.desktop').read_text())
            (package / 'koett').write_text('tampered')
            self.assertNotEqual(subprocess.run(command, check=False, capture_output=True).returncode, 0)
            self.assertEqual(target, (prefix / 'bin/koett').resolve())


if __name__ == '__main__':
    unittest.main()
