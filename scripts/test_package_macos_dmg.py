import importlib.util
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('dmg', Path(__file__).with_name('package-macos-dmg.py'))
dmg = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dmg)


class DiskImageTests(unittest.TestCase):
    def app(self, directory):
        app = Path(directory) / 'Koett.app'
        (app / 'Contents/MacOS').mkdir(parents=True)
        (app / 'Contents/Info.plist').write_bytes(plistlib.dumps({
            'CFBundleIdentifier': 'com.olledyberg.Koett',
            'CFBundleShortVersionString': '0.3.1', 'CFBundleVersion': '4'}))
        binary = app / 'Contents/MacOS/koett'
        binary.write_bytes(b'fixture')
        binary.chmod(0o755)
        return app

    def test_tree_manifest_detects_changed_permissions_and_preserves_links(self):
        with tempfile.TemporaryDirectory() as directory:
            app = self.app(directory)
            framework = app / 'Contents/Frameworks/Sparkle.framework'
            framework.mkdir(parents=True)
            (framework / 'Current').symlink_to('Versions/B')
            before = dmg.tree_manifest(app)
            self.assertEqual(before['Contents/Frameworks/Sparkle.framework/Current']['link'], 'Versions/B')
            (app / 'Contents/MacOS/koett').chmod(0o644)
            self.assertNotEqual(before, dmg.tree_manifest(app))

    def test_refuses_existing_output_before_running_tools(self):
        with tempfile.TemporaryDirectory() as directory:
            app = self.app(directory)
            output = Path(directory) / 'existing.dmg'
            output.write_bytes(b'keep me')
            with patch.object(dmg, 'run') as run, self.assertRaises(ValueError):
                dmg.package(app, output, 'identity')
            run.assert_not_called()
            self.assertEqual(output.read_bytes(), b'keep me')

    def test_refuses_existing_receipt_without_overwriting_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            app = self.app(directory)
            output = Path(directory) / 'candidate.dmg'
            receipt = output.with_suffix('.receipt.json')
            receipt.write_bytes(b'keep prior evidence')
            with patch.object(dmg, 'run') as run, self.assertRaises(ValueError):
                dmg.package(app, output, 'identity')
            run.assert_not_called()
            self.assertEqual(receipt.read_bytes(), b'keep prior evidence')

    def test_refuses_lab_build_before_signing_or_submitting(self):
        with tempfile.TemporaryDirectory() as directory:
            app = self.app(directory)
            info = app / 'Contents/Info.plist'
            contents = plistlib.loads(info.read_bytes())
            contents['KoettLocalUpdateTesting'] = True
            info.write_bytes(plistlib.dumps(contents))
            with patch.object(dmg, 'run') as run, self.assertRaises(ValueError):
                dmg.validate_app(app)
            run.assert_not_called()

    def test_refuses_wrong_identity_and_unnotarized_app(self):
        with tempfile.TemporaryDirectory() as directory:
            app = self.app(directory)
            for details in ['Signature=adhoc', 'Authority=Developer ID Application: other\nTeamIdentifier=OTHER']:
                with patch.object(dmg, 'run', side_effect=['', details]), self.assertRaises(ValueError):
                    dmg.validate_app(app)
            with patch.object(dmg, 'run', side_effect=['', 'Authority=Developer ID Application: Koett\nTeamIdentifier=LF8KF3G42Q', '', 'source=Unnotarized Developer ID']), self.assertRaises(ValueError):
                dmg.validate_app(app)


if __name__ == '__main__':
    unittest.main()
