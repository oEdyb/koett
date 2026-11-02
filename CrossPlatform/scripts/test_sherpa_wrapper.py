import hashlib
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / 'vendor/sherpa-onnx'


class SherpaWrapperTests(unittest.TestCase):
    def test_only_reviewed_upstream_source_is_modified(self):
        provenance = json.loads((VENDOR / 'koett-provenance.json').read_text())
        self.assertEqual(provenance['upstream_crate_sha256'],
                         '352a5dbbd9623e800be27e77de514e87fcfe2120256acc4434e6ea87a56bc885')
        self.assertEqual(provenance['source_commit'], '3dc7c569f31ca2cd4a20ed6f7db780327e6714c5')
        self.assertEqual(provenance['modified_files'], ['src/lib.rs'])
        self.assertEqual(set(provenance['patched_files']), {'src/lib.rs'})
        actual = {str(p.relative_to(VENDOR)) for p in (VENDOR / 'src').rglob('*') if p.is_file()}
        actual.update(['Cargo.toml', 'Cargo.toml.orig', 'Cargo.lock', '.cargo_vcs_info.json',
                       'LICENSE', 'README.md'])
        self.assertEqual(actual, set(provenance['upstream_files']))
        expected = {**provenance['upstream_files'], **provenance['patched_files']}
        for name, digest in expected.items():
            with self.subTest(path=name):
                self.assertEqual(hashlib.sha256((VENDOR / name).read_bytes()).hexdigest(), digest)

    def test_distributed_patch_notice_matches_the_source_notice(self):
        notice = ROOT / 'packaging/notices/rust/sherpa-onnx-1.13.5/KOETT-MODIFICATIONS.txt'
        self.assertEqual(notice.read_bytes(), (VENDOR / 'KOETT-PATCH.md').read_bytes())


if __name__ == '__main__':
    unittest.main()
