"""Exercise installer integrity, quoted desktop paths and atomic replacement."""
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import time
import unittest
from pathlib import Path, PureWindowsPath
from unittest import mock
from unittest.mock import MagicMock, Mock

from test_package_linux_archive import CANONICAL_NOTICES, verify_notices
from build_asr_native import SOURCE_COMMIT
from shared_context_patch import PATCHED_SHA256

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('installer', ROOT / 'packaging/linux/install.py')
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class PackageTests(unittest.TestCase):
    def test_archived_notices_accept_windows_path_separators(self):
        package = MagicMock()
        folder = package.__truediv__.return_value
        folder.__truediv__.side_effect = lambda name: CANONICAL_NOTICES / name
        files = []
        for path in CANONICAL_NOTICES.rglob('*'):
            if path.is_file() and path != CANONICAL_NOTICES / 'files.json':
                wrapped = Mock(wraps=path)
                wrapped.relative_to.return_value = PureWindowsPath(
                    path.relative_to(CANONICAL_NOTICES).as_posix())
                files.append(wrapped)
        folder.rglob.return_value = files
        verify_notices(package)

    def test_archived_notices_reject_omission_and_tampering(self):
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory)
            folder = package / 'notices'
            shutil.copytree(CANONICAL_NOTICES, folder)
            verify_notices(package)
            target = folder / 'renderer/katex-font-OFL-1.1.txt'
            original = target.read_bytes()
            target.unlink()
            with self.assertRaisesRegex(ValueError, 'file set differs'):
                verify_notices(package)
            target.write_text('Missing original license terms')
            with self.assertRaisesRegex(ValueError, 'content mismatch'):
                verify_notices(package)
            target.write_bytes(original)
            (folder / 'files.json').write_text('{}')
            with self.assertRaisesRegex(ValueError, 'index differs'):
                verify_notices(package)

    def test_system_archive_has_complete_installer(self):
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / 'native.json'
            receipt.write_text(json.dumps({'source_commit': SOURCE_COMMIT, 'shared_context_sha256': PATCHED_SHA256}))
            result = subprocess.run(['python3', str(ROOT / 'scripts/package_linux.py'), '--flavor', 'system-runtime', '--binary', '/bin/true', '--native-build-receipt', str(receipt), '--output', directory], check=True, capture_output=True, text=True)
            import tarfile
            with tarfile.open(json.loads(result.stdout)['archive']) as archive:
                members = list(archive)
                candidate, = [item for item in members if Path(item.name).name == 'candidate.json']
                self.assertEqual(json.load(archive.extractfile(candidate))['kind'], 'linux-preview-candidate')
                names = {Path(item.name).name for item in members}
                for item in members:
                    if Path(item.name).name in ['install.sh', 'koett'] and item.isfile():
                        self.assertTrue(item.mode & 0o111, item.name)
            self.assertTrue({'install.py', 'install.sh', 'preflight.py', 'files.json', 'SYSTEM-RUNTIME.md', 'koett'}.issubset(names))
            self.assertFalse(any(name.endswith('.a') for name in names))

    def test_missing_or_stale_native_receipts_fail_before_creating_a_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            receipt = root / 'native.json'
            output = root / 'packages'
            command = ['python3', str(ROOT / 'scripts/package_linux.py'), '--flavor', 'system-runtime', '--binary', '/bin/true', '--native-build-receipt', str(receipt), '--output', str(output)]
            for value in [None, [], {'source_commit': SOURCE_COMMIT}, {'source_commit': SOURCE_COMMIT, 'shared_context_sha256': 'stale'}]:
                with self.subTest(value=value):
                    if value is not None:
                        receipt.write_text(json.dumps(value))
                    result = subprocess.run(command, capture_output=True, text=True)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn('rerun scripts/build_asr_native.py', result.stderr)
                    self.assertFalse(output.exists())

    def test_current_pc_still_requires_all_runtime_inputs(self):
        result = subprocess.run(['python3', str(ROOT / 'scripts/package_linux.py'), '--output', '/unused'], check=False, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('requires --gtk-runtime, --asr-runtime and --llama-runtime', result.stderr)


class InstallTests(unittest.TestCase):
    def test_prefix_inside_package_is_rejected_before_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory) / 'package'
            package.mkdir()
            script = package / 'install.py'
            script.write_bytes((ROOT / 'packaging/linux/install.py').read_bytes())
            alias = Path(directory) / 'package-link'
            alias.symlink_to(package, target_is_directory=True)
            for prefix in [package, package / 'local', alias / 'local']:
                with self.subTest(prefix=prefix):
                    result = subprocess.run(['python3', str(script), '--prefix', str(prefix)], check=False, capture_output=True, text=True)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn('outside the extracted package', result.stderr)
                    self.assertEqual({p.name for p in package.iterdir()}, {'install.py'})

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

    def package_fixture(self, root):
        package = root / 'package'
        package.mkdir()
        for name in ['install.py', 'com.olledyberg.Koett.desktop', 'com.olledyberg.Koett.svg']:
            (package / name).write_bytes((ROOT / 'packaging/linux' / name).read_bytes())
        (package / 'koett').write_text('fixture; never executed')
        (package / 'files.json').write_text(json.dumps({p.name: installer.sha(p) for p in package.iterdir()}))
        return package

    def test_a_failed_desktop_write_leaves_the_launcher_alone(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = self.package_fixture(root)
            prefix = root / 'prefix'
            command = ['python3', str(package / 'install.py'), '--prefix', str(prefix)]
            subprocess.run(command, check=True, capture_output=True)
            first = (prefix / 'bin/koett').resolve()
            (package / 'koett').write_text('next version; never executed')
            (package / 'files.json').write_text(json.dumps({p.name: installer.sha(p) for p in package.iterdir() if p.name != 'files.json'}))
            # A file where the icon folder should be makes the icon write fail.
            icons = prefix / 'share/icons'
            shutil.rmtree(icons)
            icons.write_text('in the way')
            self.assertNotEqual(subprocess.run(command, check=False, capture_output=True).returncode, 0)
            self.assertEqual(first, (prefix / 'bin/koett').resolve())

    def test_files_reach_the_disk_before_any_name_points_at_them(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = self.package_fixture(root)
            prefix = root / 'prefix'
            events = []
            real_fsync, real_rename, real_replace = os.fsync, Path.rename, Path.replace
            def fsync(descriptor):
                events.append(('flush', os.readlink(f'/proc/self/fd/{descriptor}')))
                return real_fsync(descriptor)
            def rename(path, target):
                events.append(('rename', str(path), str(target)))
                return real_rename(path, target)
            def replace(path, target):
                events.append(('replace', str(path), str(target)))
                return real_replace(path, target)
            argv = ['install.py', '--prefix', str(prefix)]
            spec = importlib.util.spec_from_file_location('package_installer', package / 'install.py')
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            with mock.patch('sys.argv', argv), mock.patch('os.fsync', fsync), \
                    mock.patch.object(Path, 'rename', rename), mock.patch.object(Path, 'replace', replace), \
                    mock.patch('sys.stdout'):
                module.main()
            moved = next(i for i, e in enumerate(events) if e[0] == 'rename')
            flushed = {e[1] for e in events[:moved] if e[0] == 'flush'}
            staging = events[moved][1]
            self.assertIn(f'{staging}/koett', flushed)
            self.assertIn(staging, flushed)
            self.assertIn(('flush', str(prefix / 'lib/koett/versions')), events[moved:])
            swap = next(i for i, e in enumerate(events) if e[0] == 'replace' and e[2] == str(prefix / 'bin/koett'))
            self.assertEqual(events[swap + 1], ('flush', str(prefix / 'bin')))
            self.assertTrue(any(e[0] == 'replace' and e[2].endswith('.svg') for e in events[:swap]))


if __name__ == '__main__':
    unittest.main()
