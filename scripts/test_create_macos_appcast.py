import base64
import importlib.util
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET
import zipfile

spec = importlib.util.spec_from_file_location('appcast', Path(__file__).with_name('create-macos-appcast.py'))
appcast = importlib.util.module_from_spec(spec)
spec.loader.exec_module(appcast)


class AppcastTests(unittest.TestCase):
    def archive(self, directory, **overrides):
        key = plistlib.loads((Path(__file__).resolve().parent.parent / 'Packaging/Info.plist').read_bytes())['SUPublicEDKey']
        info = dict(CFBundleIdentifier='com.olledyberg.Koett', CFBundleVersion='4',
            CFBundleShortVersionString='0.3.1', KoettSourceCommit='a' * 40,
            LSMinimumSystemVersion='15.0', SUFeedURL=appcast.FEED, SUPublicEDKey=key,
            SURequireSignedFeed=True, SUVerifyUpdateBeforeExtraction=True,
            SUSignedFeedFailureExpirationInterval=0)
        info.update(overrides)
        archive = Path(directory) / 'Koett-0.3.1-macOS-arm64.zip'
        with zipfile.ZipFile(archive, 'w') as zipped:
            zipped.writestr('Koett.app/Contents/Info.plist', plistlib.dumps(info))
        return archive

    def test_metadata_binds_exact_production_configuration_and_source(self):
        with tempfile.TemporaryDirectory() as directory:
            info = appcast.metadata(self.archive(directory))
            self.assertEqual(info['CFBundleVersion'], '4')
            self.assertEqual(info['KoettSourceCommit'], 'a' * 40)

    def test_lab_wrong_key_and_unsigned_fallback_are_refused_before_signing(self):
        for invalid in [dict(KoettLocalUpdateTesting=True), dict(SUPublicEDKey=base64.b64encode(b'x'*32).decode()),
                        dict(SUSignedFeedFailureExpirationInterval=20), dict(SURequireSignedFeed=False),
                        dict(SUFeedURL='http://127.0.0.1/feed'), dict(KoettSourceCommit='dirty')]:
            with self.subTest(invalid=invalid), tempfile.TemporaryDirectory() as directory:
                archive = self.archive(directory, **invalid)
                with patch.object(appcast.subprocess, 'check_output') as signer, self.assertRaises(ValueError):
                    appcast.create(archive, Path(directory)/'appcast.xml', Path('/unused-signer'))
                signer.assert_not_called()

    def test_archive_name_cannot_be_a_mutable_latest_asset(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = self.archive(directory)
            moved = archive.rename(Path(directory)/'Koett-latest.zip')
            with self.assertRaises(ValueError): appcast.metadata(moved)

    def test_feed_contains_immutable_archive_build_minimum_os_and_signature(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = self.archive(directory); info = appcast.metadata(archive)
            signature = base64.b64encode(b'x'*64).decode()
            body = appcast.unsigned_feed(info, archive, signature, archive.stat().st_size)
            item = ET.fromstring(body).find('channel/item')
            self.assertEqual(item.find('{'+appcast.SPARKLE+'}version').text, '4')
            self.assertEqual(item.find('{'+appcast.SPARKLE+'}minimumSystemVersion').text, '15.0')
            enclosure = item.find('enclosure')
            self.assertEqual(enclosure.attrib['url'], 'https://github.com/oEdyb/koett/releases/download/v0.3.1/Koett-0.3.1-macOS-arm64.zip')
            self.assertEqual(enclosure.attrib['{'+appcast.SPARKLE+'}edSignature'], signature)
            self.assertEqual(enclosure.attrib['length'], str(archive.stat().st_size))

    def test_wrong_length_and_invalid_signature_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = self.archive(directory); info = appcast.metadata(archive)
            for signature, length in [('invalid', archive.stat().st_size), (base64.b64encode(b'x'*64).decode(), 0)]:
                with self.assertRaises(ValueError): appcast.unsigned_feed(info, archive, signature, length)

    def test_existing_signed_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)/'appcast.xml';output.write_bytes(b'keep signed bytes')
            with patch.object(appcast.subprocess, 'check_output') as signer, self.assertRaises(ValueError):
                appcast.create(Path('/unused.zip'), output, Path('/unused-signer'))
            signer.assert_not_called();self.assertEqual(output.read_bytes(), b'keep signed bytes')


if __name__ == '__main__': unittest.main()
