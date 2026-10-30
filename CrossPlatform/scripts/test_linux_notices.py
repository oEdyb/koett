import hashlib
import json
import shutil
import tempfile
import unittest
from pathlib import Path

from check_linux_notices import ROOT, validate


class LinuxNoticeTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name) / 'CrossPlatform'
        self.folder = self.root / 'packaging/notices'
        shutil.copytree(ROOT / 'packaging/notices', self.folder)
        font_path = Path('Sources/Koett/Resources/AssistantRenderer/fonts')
        shutil.copytree(ROOT.parent / font_path, self.root.parent / font_path)

    def reindex(self):
        files = [
            {'path': str(path.relative_to(self.folder)),
             'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
             'bytes': path.stat().st_size}
            for path in sorted(self.folder.rglob('*'))
            if path.is_file() and path != self.folder / 'files.json'
        ]
        (self.folder / 'files.json').write_text(json.dumps({'schema': 1, 'files': files}))

    def test_reviewed_notices_validate(self):
        self.assertGreater(validate(self.root), 600)

    def test_font_license_required_even_if_reindexed(self):
        (self.folder / 'renderer/katex-font-OFL-1.1.txt').unlink()
        self.reindex()
        with self.assertRaisesRegex(ValueError, 'required notice missing'):
            validate(self.root)

    def test_eigen_source_patch_required_even_if_reindexed(self):
        (self.folder / 'native/onnxruntime/build-provenance/cmake/patches/eigen/s390x-build.patch').unlink()
        self.reindex()
        with self.assertRaisesRegex(ValueError, 'required notice missing'):
            validate(self.root)

    def test_original_notice_content_bound_to_component_provenance(self):
        (self.folder / 'renderer/marked-LICENSE.txt').write_text('replacement license')
        self.reindex()
        with self.assertRaisesRegex(ValueError, 'notice provenance mismatch'):
            validate(self.root)

    def test_full_kissfft_license_required_even_if_reindexed(self):
        (self.folder / 'native/kissfft/LICENSES/BSD-3-Clause').unlink()
        self.reindex()
        with self.assertRaisesRegex(ValueError, 'required notice missing'):
            validate(self.root)

    def test_authored_source_access_and_attribution_bound_to_review(self):
        for name in ['native/onnxruntime/SOURCE-ACCESS.md',
                     'native/onnxruntime/provenance.json', 'models/NEMOTRON-ATTRIBUTION.md']:
            with self.subTest(name=name):
                path = self.folder / name
                original = path.read_bytes()
                path.write_text('Reviewed source access or attribution removed.\n')
                self.reindex()
                with self.assertRaisesRegex(ValueError, 'notice provenance mismatch'):
                    validate(self.root)
                path.write_bytes(original)

    def test_font_update_requires_reviewed_metadata(self):
        font = next((self.root.parent / 'Sources/Koett/Resources/AssistantRenderer/fonts').glob('*.woff2'))
        font.write_bytes(b'new font')
        with self.assertRaisesRegex(ValueError, 'embedded font differs'):
            validate(self.root)

    def test_added_font_requires_reviewed_metadata(self):
        font = self.root.parent / 'Sources/Koett/Resources/AssistantRenderer/fonts/new.woff2'
        font.write_bytes(b'new font')
        with self.assertRaisesRegex(ValueError, 'embedded font set differs'):
            validate(self.root)


if __name__ == '__main__':
    unittest.main()
